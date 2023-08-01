use chrono::DateTime;
use chrono::NaiveDate;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fmt::Display;

pub mod priority;
use crate::color;
use crate::config::Config;
use crate::projects;
use crate::tasks::priority::Priority;
use crate::{input, time, todoist};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Task {
    pub id: String,
    pub content: String,
    pub priority: Priority,
    pub description: String,
    pub due: Option<DateInfo>,
    /// Only on rest api return value
    pub is_completed: Option<bool>,
    pub is_deleted: Option<bool>,
    /// only on sync api return value
    pub checked: Option<bool>,
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct DateInfo {
    pub date: String,
    pub is_recurring: bool,
    pub timezone: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Body {
    items: Vec<Task>,
}

pub enum FormatType {
    List,
    Single,
}

enum DateTimeInfo {
    NoDateTime,
    Date {
        date: NaiveDate,
        is_recurring: bool,
    },
    DateTime {
        datetime: DateTime<Tz>,
        is_recurring: bool,
    },
}

impl Task {
    pub fn fmt(&self, config: &Config, format: FormatType) -> String {
        let content = match self.priority {
            priority::Priority::Low => color::blue_string(&self.content),
            priority::Priority::Medium => color::yellow_string(&self.content),
            priority::Priority::High => color::red_string(&self.content),
            priority::Priority::None => color::normal_string(&self.content),
        };

        let buffer = match format {
            FormatType::List => String::from("  "),
            FormatType::Single => String::from(""),
        };

        let description = match &*self.description {
            "" => String::from(""),
            _ => format!("\n{buffer}{}", self.description),
        };
        let due = match &self.datetimeinfo(config) {
            Ok(DateTimeInfo::Date { date, is_recurring }) => {
                let recurring_icon = if *is_recurring { " ↻" } else { "" };
                let date_string = time::format_date(date, config);

                format!("\n{buffer}Due: {date_string}{recurring_icon}")
            }
            Ok(DateTimeInfo::DateTime {
                datetime,
                is_recurring,
            }) => {
                let recurring_icon = if *is_recurring { " ↻" } else { "" };
                let datetime_string = time::format_datetime(datetime, config);

                format!("\n{buffer}Due: {datetime_string}{recurring_icon}")
            }
            Ok(DateTimeInfo::NoDateTime) => String::from(""),
            Err(string) => string.clone(),
        };

        let prefix = match format {
            FormatType::List => String::from("- "),
            FormatType::Single => String::from(""),
        };
        format!("{prefix}{content}{description}{due}")
    }

    /// Determines the numeric value of an task for sorting
    fn value(&self, config: &Config) -> u32 {
        let date_value: u8 = self.date_value(config);
        let priority_value: u8 = self.priority_value();

        date_value as u32 + priority_value as u32
    }

    /// Return the value of the due field
    fn date_value(&self, config: &Config) -> u8 {
        match &self.datetimeinfo(config) {
            Ok(DateTimeInfo::NoDateTime) => 80,
            Ok(DateTimeInfo::Date { date, is_recurring }) => {
                let today_value = if *date == time::today_date(config) {
                    100
                } else {
                    0
                };
                let overdue_value = if self.is_overdue(config) { 150 } else { 0 };
                let recurring_value = if is_recurring.to_owned() { 0 } else { 50 };
                today_value + overdue_value + recurring_value
            }
            Ok(DateTimeInfo::DateTime {
                datetime,
                is_recurring,
            }) => {
                let recurring_value = if is_recurring.to_owned() { 0 } else { 50 };
                let duration = *datetime - time::now(config);
                match duration.num_minutes() {
                    -15..=15 => 200 + recurring_value,
                    _ => recurring_value,
                }
            }
            Err(_) => 50,
        }
    }

    /// Return the value of the due field
    fn datetime(&self, config: &Config) -> Option<DateTime<Tz>> {
        match self.datetimeinfo(config) {
            Ok(DateTimeInfo::DateTime { datetime, .. }) => Some(datetime),
            _ => None,
        }
    }

    fn priority_value(&self) -> u8 {
        match &self.priority {
            Priority::None => 2,
            Priority::Low => 1,
            Priority::Medium => 3,
            Priority::High => 4,
        }
    }

    /// Converts the JSON date representation into Date or Datetime
    fn datetimeinfo(&self, config: &Config) -> Result<DateTimeInfo, String> {
        let tz = match (self.clone().due, config.clone().timezone) {
            (None, Some(tz_string)) => time::timezone_from_str(&Some(tz_string)),
            (None, None) => Tz::UTC,
            (Some(DateInfo { timezone: None, .. }), Some(tz_string)) => time::timezone_from_str(&Some(tz_string)),
            (Some(DateInfo { timezone: None, .. }), None) => Tz::UTC,
            (Some(DateInfo {
                timezone: Some(tz_string),
                ..
                // Remove the Some here
            }), _) => time::timezone_from_str(&Some(tz_string)),
        };
        match self.clone().due {
            None => Ok(DateTimeInfo::NoDateTime),
            Some(DateInfo {
                date, is_recurring, ..
            }) if date.len() == 10 => Ok(DateTimeInfo::Date {
                date: time::date_from_str(&date, tz)?,
                is_recurring,
            }),
            Some(DateInfo {
                date, is_recurring, ..
            }) => Ok(DateTimeInfo::DateTime {
                datetime: time::datetime_from_str(&date, tz)?,
                is_recurring,
            }),
        }
    }

    pub fn filter(&self, config: &Config, filter: &projects::TaskFilter) -> bool {
        match filter {
            projects::TaskFilter::Unscheduled => self.has_no_date() || self.is_overdue(config),
            projects::TaskFilter::Overdue => self.is_overdue(config),
            projects::TaskFilter::Recurring => self.is_recurring(),
        }
    }

    fn has_no_date(&self) -> bool {
        self.due.is_none()
    }

    // Returns true if the datetime is today and there is a time
    fn is_today(&self, config: &Config) -> bool {
        match self.datetimeinfo(config) {
            Ok(DateTimeInfo::NoDateTime) => false,
            Ok(DateTimeInfo::Date { date, .. }) => date == time::today_date(config),
            Ok(DateTimeInfo::DateTime { datetime, .. }) => {
                time::datetime_is_today(datetime, config)
            }
            Err(_) => false,
        }
    }

    fn is_overdue(&self, config: &Config) -> bool {
        match self.clone().datetimeinfo(config) {
            Ok(DateTimeInfo::NoDateTime) => false,
            Ok(DateTimeInfo::Date { date, .. }) => time::is_date_in_past(date, config),
            Ok(DateTimeInfo::DateTime { datetime, .. }) => {
                time::is_date_in_past(datetime.date_naive(), config)
            }
            Err(_) => false,
        }
    }

    /// Returns true if it is a recurring task
    fn is_recurring(&self) -> bool {
        match self.due {
            None => false,
            Some(DateInfo { is_recurring, .. }) => is_recurring,
        }
    }

    /// Returns true when it is a datetime, otherwise false
    fn has_time(&self, config: &Config) -> bool {
        matches!(
            self.clone().datetimeinfo(config),
            Ok(DateTimeInfo::DateTime { .. })
        )
    }
}

pub fn json_to_tasks(json: String) -> Result<Vec<Task>, String> {
    let result: Result<Body, _> = serde_json::from_str(&json);
    match result {
        Ok(body) => Ok(body.items),
        Err(err) => Err(format!("Could not parse response for task: {err:?}")),
    }
}

pub fn json_to_task(json: String) -> Result<Task, String> {
    match serde_json::from_str(&json) {
        Ok(task) => Ok(task),
        Err(err) => Err(format!("Could not parse response for task: {err:?}")),
    }
}

pub fn sort_by_value(mut tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks.sort_by_key(|b| Reverse(b.value(config)));
    tasks
}

pub fn sort_by_datetime(mut tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks.sort_by_key(|i| i.datetime(config));
    tasks
}

pub fn filter_not_in_future(tasks: Vec<Task>, config: &Config) -> Result<Vec<Task>, String> {
    let tasks = tasks
        .into_iter()
        .filter(|task| task.is_today(config) || task.has_no_date() || task.is_overdue(config))
        .collect();

    Ok(tasks)
}

pub fn filter_today_and_has_time(tasks: Vec<Task>, config: &Config) -> Vec<Task> {
    tasks
        .into_iter()
        .filter(|task| task.is_today(config) && task.has_time(config))
        .collect()
}

pub fn set_priority(config: &Config, task: Task) -> Result<String, String> {
    println!("{}", task.fmt(config, FormatType::Single));

    let options = vec![
        Priority::None,
        Priority::Low,
        Priority::Medium,
        Priority::High,
    ];
    let priority = input::select(
        "Choose a priority that should be assigned to task: ",
        options,
        config.mock_select,
    )?;

    let config = config.set_next_id(&task.id);
    todoist::update_task_priority(config, task, priority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use pretty_assertions::assert_eq;

    #[test]
    fn date_value_can_handle_date() {
        let config = test::fixtures::config();
        // On another day
        assert_eq!(test::fixtures::task().date_value(&config), 50);

        // Recurring
        let task = Task {
            due: Some(DateInfo {
                is_recurring: true,
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };
        assert_eq!(task.date_value(&config), 0);

        // Overdue
        let task = Task {
            due: Some(DateInfo {
                date: String::from("2001-11-13"),
                is_recurring: true,
                timezone: Some(String::from("America/Los_Angeles")),
            }),
            ..test::fixtures::task()
        };
        assert_eq!(task.date_value(&config), 150);

        // No date
        let task = Task { due: None, ..task };
        assert_eq!(task.date_value(&config), 80);
    }

    #[test]
    fn date_value_can_handle_datetime() {
        let config = test::fixtures::config();
        let task = Task {
            due: Some(DateInfo {
                date: String::from("2021-02-27T19:41:56Z"),
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };

        assert_eq!(task.date_value(&config), 50);
    }

    #[test]
    fn can_format_task_with_a_date() {
        let config = test::fixtures::config();
        let task = Task {
            content: String::from("Get gifts for the twins"),
            due: Some(DateInfo {
                date: String::from("2021-08-13"),
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };

        assert_eq!(
            format!("{}", task.fmt(&config, FormatType::Single)),
            "Get gifts for the twins\nDue: 2021-08-13"
        );
    }

    #[test]
    fn can_format_task_with_today() {
        let config = test::fixtures::config();
        let task = Task {
            content: String::from("Get gifts for the twins"),
            due: Some(DateInfo {
                date: time::today_string(&config),
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };

        assert_eq!(
            format!("{}", task.fmt(&config, FormatType::Single)),
            "Get gifts for the twins\nDue: Today"
        );
    }

    #[test]
    fn value_can_get_the_value_of_an_task() {
        let config = test::fixtures::config();
        let task = Task {
            due: Some(DateInfo {
                date: String::from("2021-09-06T16:00:00"),
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };

        assert_matches!(task.datetime(&config), Some(DateTime { .. }));
    }

    #[test]
    fn datetime_works_with_date() {
        let config = test::fixtures::config();
        let task = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };

        assert_eq!(task.datetime(&config), None);
    }

    #[test]
    fn has_no_date_works() {
        let config = test::fixtures::config();
        let task = Task {
            due: None,
            ..test::fixtures::task()
        };

        assert!(task.has_no_date());

        let task_today = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                ..test::fixtures::task().due.unwrap()
            }),
            ..test::fixtures::task()
        };
        assert!(!task_today.has_no_date());
    }

    #[test]
    fn has_time_works() {
        let config = test::fixtures::config();
        let task = Task {
            due: None,
            ..test::fixtures::task()
        };

        assert!(!task.has_time(&config));

        let task_with_date = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                is_recurring: false,
                timezone: None,
            }),
            ..task.clone()
        };
        assert!(!task_with_date.has_time(&config));

        let task_with_datetime = Task {
            due: Some(DateInfo {
                date: String::from("2021-09-06T16:00:00"),
                is_recurring: false,
                timezone: None,
            }),
            ..task
        };
        assert!(task_with_datetime.has_time(&config));
    }

    #[test]
    fn is_today_works() {
        let config = test::fixtures::config();
        let task = Task {
            due: None,
            ..test::fixtures::task()
        };

        assert!(!task.is_today(&config));

        let task_today = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                is_recurring: false,
                timezone: None,
            }),
            ..test::fixtures::task()
        };
        assert!(task_today.is_today(&config));

        let task_in_past = Task {
            due: Some(DateInfo {
                date: String::from("2021-09-06T16:00:00"),
                is_recurring: false,
                timezone: None,
            }),
            ..test::fixtures::task()
        };
        assert!(!task_in_past.is_today(&config));
    }

    #[test]
    fn sort_by_value_works() {
        let config = test::fixtures::config();
        let today = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                is_recurring: false,
                timezone: None,
            }),
            ..test::fixtures::task()
        };

        let today_recurring = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                is_recurring: false,
                timezone: None,
            }),
            ..test::fixtures::task()
        };

        let future = Task {
            due: Some(DateInfo {
                date: String::from("2035-12-12"),
                is_recurring: false,
                timezone: None,
            }),
            ..test::fixtures::task()
        };

        let input = vec![future.clone(), today_recurring.clone(), today.clone()];
        let result = vec![today, today_recurring, future];

        assert_eq!(sort_by_value(input, &config), result);
    }

    #[test]
    fn sort_by_datetime_works() {
        let config = test::fixtures::config();
        let no_date = Task {
            id: String::from("222"),
            content: String::from("Get gifts for the twins"),
            checked: None,
            description: String::from(""),
            due: None,
            priority: Priority::Medium,
            is_deleted: None,
            is_completed: None,
        };

        let date_not_datetime = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                is_recurring: false,
                timezone: None,
            }),
            ..no_date.clone()
        };

        let present = Task {
            due: Some(DateInfo {
                date: String::from("2020-09-06T16:00:00"),
                is_recurring: false,
                timezone: None,
            }),
            ..no_date.clone()
        };

        let future = Task {
            due: Some(DateInfo {
                date: String::from("2035-09-06T16:00:00"),
                is_recurring: false,
                timezone: None,
            }),
            ..no_date.clone()
        };

        let past = Task {
            due: Some(DateInfo {
                date: String::from("2015-09-06T16:00:00"),
                is_recurring: false,
                timezone: None,
            }),
            ..no_date.clone()
        };

        let input = vec![
            future.clone(),
            past.clone(),
            present.clone(),
            no_date.clone(),
            date_not_datetime.clone(),
        ];
        let result = vec![no_date, date_not_datetime, past, present, future];

        assert_eq!(sort_by_datetime(input, &config), result);
    }

    #[test]
    fn is_overdue_works() {
        let config = test::fixtures::config();
        let task = Task {
            id: String::from("222"),
            content: String::from("Get gifts for the twins"),
            checked: None,
            description: String::from(""),
            due: None,
            priority: Priority::Medium,
            is_deleted: None,
            is_completed: None,
        };

        assert!(!task.is_overdue(&config));

        let task_today = Task {
            due: Some(DateInfo {
                date: time::today_string(&config),
                is_recurring: false,
                timezone: None,
            }),
            ..task.clone()
        };
        assert!(!task_today.is_overdue(&config));

        let task_future = Task {
            due: Some(DateInfo {
                date: String::from("2035-12-12"),
                is_recurring: false,
                timezone: None,
            }),
            ..task.clone()
        };
        assert!(!task_future.is_overdue(&config));

        let task_today = Task {
            due: Some(DateInfo {
                date: String::from("2020-12-20"),
                is_recurring: false,
                timezone: None,
            }),
            ..task
        };
        assert!(task_today.is_overdue(&config));
    }

    #[test]
    fn json_to_tasks_works() {
        let json = String::from("2{.e");
        let error_text = String::from("Could not parse response for task: Error(\"invalid type: integer `2`, expected struct Body\", line: 1, column: 1)");
        assert_eq!(json_to_tasks(json), Err(error_text));
    }

    #[test]
    fn json_to_task_works() {
        let json = String::from("2{.e");
        let error_text = String::from("Could not parse response for task: Error(\"invalid type: integer `2`, expected struct Task\", line: 1, column: 1)");
        assert_eq!(json_to_task(json), Err(error_text));
    }

    #[test]
    fn test_to_integer() {
        assert_eq!(Priority::None.to_integer(), 4);
        assert_eq!(Priority::Low.to_integer(), 3);
        assert_eq!(Priority::Medium.to_integer(), 2);
        assert_eq!(Priority::High.to_integer(), 1);
    }

    #[test]
    fn test_set_priority() {
        let task = test::fixtures::task();
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/rest/v2/tasks/222")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(test::responses::task())
            .create();
        let config = test::fixtures::config()
            .mock_select(1)
            .mock_url(server.url());

        let result = set_priority(&config, task);
        assert_eq!(result, Ok("✓".to_string()));
        mock.assert();
    }
}