use crate::{
    color,
    config::Config,
    input,
    tasks::{self, FormatType, Task},
    todoist,
};

/// All tasks for a project
pub fn all_tasks(config: &Config, filter: &String) -> Result<String, String> {
    let tasks = todoist::tasks_for_filter(config, filter)?;

    if tasks.is_empty() {
        return Ok(format!("No tasks for filter: '{filter}'"));
    }

    let mut buffer = String::new();
    buffer.push_str(&color::green_string(&format!(
        "Tasks for filter: '{filter}'"
    )));

    for task in tasks::sort_by_datetime(tasks, config) {
        buffer.push('\n');
        buffer.push_str(&task.fmt(config, FormatType::List));
    }
    Ok(buffer)
}

pub fn rename_task(config: &Config, filter: String) -> Result<String, String> {
    let project_tasks = todoist::tasks_for_filter(config, &filter)?;

    let selected_task = input::select(
        "Choose a task of the project:",
        project_tasks,
        config.mock_select,
    )?;
    let task_content = selected_task.content.as_str();

    let new_task_content = input::string_with_default("Edit the task you selected:", task_content)?;

    if task_content == new_task_content {
        return Ok(color::green_string(
            "The content is the same, no need to change it",
        ));
    }

    todoist::update_task_name(config, selected_task, new_task_content)
}

pub fn label(config: &Config, filter: &str, labels: Vec<String>) -> Result<String, String> {
    let tasks = todoist::tasks_for_filter(config, filter)?;
    for task in tasks {
        label_task(config, task, &labels)?;
    }
    Ok(color::green_string(&format!(
        "There are no more tasks for filter: '{filter}'"
    )))
}

fn label_task(config: &Config, task: Task, labels: &Vec<String>) -> Result<String, String> {
    let label = input::select(
        &format!("Select label for {task}:"),
        labels.to_owned(),
        config.mock_select,
    )?;

    todoist::add_task_label(config, task, label)
}

/// Get the next task by priority and save its id to config
pub fn next_task(config: Config, filter: &str) -> Result<String, String> {
    match fetch_next_task(&config, filter) {
        Ok(Some((task, remaining))) => {
            config.set_next_id(&task.id).save()?;
            let task_string = task.fmt(&config, FormatType::Single);
            Ok(format!("{task_string}\n{remaining} task(s) remaining"))
        }
        Ok(None) => Ok(color::green_string("No tasks on list")),
        Err(e) => Err(e),
    }
}

fn fetch_next_task(config: &Config, filter: &str) -> Result<Option<(Task, usize)>, String> {
    let tasks = todoist::tasks_for_filter(config, filter)?;
    let tasks = tasks::sort_by_value(tasks, config);

    Ok(tasks.first().map(|task| (task.to_owned(), tasks.len())))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use pretty_assertions::assert_eq;

    /// Need to adjust this value forward or back an hour when timezone changes
    const TIME: &str = "16:59";

    #[test]
    fn test_all_tasks() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/rest/v2/tasks/?filter=today")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(test::responses::rest_tasks())
            .create();

        let config = test::fixtures::config().mock_url(server.url());

        let config_with_timezone = Config {
            timezone: Some(String::from("US/Pacific")),
            mock_url: Some(server.url()),
            ..config
        };

        let filter = String::from("today");

        assert_eq!(
            all_tasks(&config_with_timezone, &filter),
            Ok(format!(
                "Tasks for filter: 'today'\n- Put out recycling\n  ! {TIME} ↻ every other mon at 16:30\n"
            ))
        );
        mock.assert();
    }

    #[test]
    fn test_rename_task() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/rest/v2/tasks/?filter=today")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(test::responses::rest_tasks())
            .create();

        let config = test::fixtures::config()
            .mock_url(server.url())
            .mock_select(0);

        let result = rename_task(&config, String::from("today"));
        assert_eq!(
            result,
            Ok("The content is the same, no need to change it".to_string())
        );
        mock.assert();
    }
    #[test]
    fn test_get_next_task() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/rest/v2/tasks/?filter=today")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(test::responses::rest_tasks())
            .create();

        let config = test::fixtures::config().mock_url(server.url());

        let config_dir = dirs::config_dir().unwrap().to_str().unwrap().to_owned();

        let config_with_timezone = Config {
            timezone: Some(String::from("US/Pacific")),
            path: format!("{config_dir}/test3"),
            mock_url: Some(server.url()),
            ..config
        };

        config_with_timezone.clone().create().unwrap();

        let filter = String::from("today");
        assert_eq!(
            next_task(config_with_timezone, &filter),
            Ok(format!(
                "Put out recycling\n! {TIME} ↻ every other mon at 16:30\n\n1 task(s) remaining"
            ))
        );
    }
    #[test]
    fn test_label() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/rest/v2/tasks/?filter=today")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(test::responses::rest_tasks())
            .create();

        let mock2 = server
            .mock("POST", "/rest/v2/tasks/999999")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(test::responses::rest_tasks())
            .create();

        let config = test::fixtures::config().mock_url(server.url());

        let config_dir = dirs::config_dir().unwrap().to_str().unwrap().to_owned();

        let config_with_timezone = Config {
            timezone: Some(String::from("US/Pacific")),
            path: format!("{config_dir}/test3"),
            mock_url: Some(server.url()),
            mock_select: Some(0),
            ..config
        };

        config_with_timezone.clone().create().unwrap();

        let filter = String::from("today");
        let labels = vec![String::from("thing")];

        assert_eq!(
            label(&config_with_timezone, &filter, labels),
            Ok(String::from("There are no more tasks for filter: 'today'"))
        );
        mock.assert();
        mock2.assert();
    }
}
