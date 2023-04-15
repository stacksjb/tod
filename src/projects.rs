use crate::config::Config;
use crate::items::Item;
use crate::{config, items, projects, request};
use colored::*;

const ADD_ERROR: &str = "Must provide project name and number, i.e. tod --add projectname 12345";

/// List the projects in config
pub fn list(config: Config) -> Result<String, String> {
    let mut projects: Vec<String> = config.projects.keys().map(|k| k.to_owned()).collect();
    if projects.is_empty() {
        return Ok(String::from("No projects found"));
    }
    projects.sort();
    let mut buffer = String::new();
    buffer.push_str(&green_string("Projects"));

    for key in projects {
        buffer.push_str("\n - ");
        buffer.push_str(&key);
    }
    Ok(buffer)
}

/// Add a project to the projects HashMap in Config
pub fn add(config: Config, params: Vec<String>) -> Result<String, String> {
    let mut params = params;
    let num = params
        .pop()
        .ok_or(ADD_ERROR)?
        .parse::<u32>()
        .or(Err(ADD_ERROR))?;

    let name = params.pop().ok_or(ADD_ERROR)?;

    config.add_project(name, num).save()
}

/// Remove a project from the projects HashMap in Config
pub fn remove(config: Config, project_name: &str) -> Result<String, String> {
    config.remove_project(project_name).save()
}

pub fn project_id(config: &Config, project_name: &str) -> Result<String, String> {
    let project_id = config
        .projects
        .get(project_name)
        .ok_or(format!(
            "Project {project_name} not found, please add it to config"
        ))?
        .to_string();

    Ok(project_id)
}

/// Get the next item by priority and save its id to config
pub fn next_item(config: Config, project_name: &str) -> Result<String, String> {
    match fetch_next_item(config.clone(), project_name) {
        Ok(Some(item)) => {
            config.set_next_id(item.id.clone()).save()?;
            Ok(item.fmt(&config))
        }
        Ok(None) => Ok(green_string("No items on list")),
        Err(e) => Err(e),
    }
}

fn fetch_next_item(config: Config, project_name: &str) -> Result<Option<Item>, String> {
    let project_id = projects::project_id(&config, project_name)?;
    let items = request::items_for_project(&config, &project_id)?;
    let filtered_items = items::filter_not_in_future(items, &config)?;
    let items = items::sort_by_value(filtered_items, &config);

    Ok(items.first().map(|item| item.to_owned()))
}

/// Get next items and give an interactive prompt for completing them
pub fn next_item_interactive(config: Config, project_name: &str) -> Result<String, String> {
    let mut config = config;
    loop {
        match fetch_next_item(config.clone(), project_name) {
            Ok(Some(item)) => {
                config.set_next_id(item.id.clone()).save()?;
                config = Config::load(&config.path)?;
                match handle_item(config.clone(), item) {
                    Some(Ok(_)) => (),
                    Some(Err(e)) => return Err(e),
                    None => return Ok(green_string("Exited")),
                }
            }
            Ok(None) => return Ok(green_string("Done")),
            Err(e) => return Err(e),
        }
    }
}
fn handle_item(config: Config, item: Item) -> Option<Result<String, String>> {
    let options = vec!["complete", "quit"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    println!("{}", item.fmt(&config));
    match config::select_input("Select an option", options) {
        Ok(string) => {
            if string == *"complete" {
                Some(request::complete_item(config))
            } else {
                None
            }
        }
        Err(e) => Some(Err(e)),
    }
}

// Scheduled that are today and have a time on them (AKA appointments)
pub fn scheduled_items(config: &Config, project_name: &str) -> Result<String, String> {
    let project_id = projects::project_id(config, project_name)?;

    let items = request::items_for_project(config, &project_id)?;
    let filtered_items = items::filter_today_and_has_time(items, config);

    if filtered_items.is_empty() {
        return Ok(String::from("No scheduled items found"));
    }

    let mut buffer = String::new();
    buffer.push_str(&green_string(&format!("Schedule for {project_name}")));

    for item in items::sort_by_datetime(filtered_items, config) {
        buffer.push('\n');
        buffer.push_str(&item.fmt(config));
    }
    Ok(buffer)
}

/// All items for a project
pub fn all_items(config: &Config, project_name: &str) -> Result<String, String> {
    let project_id = projects::project_id(config, project_name)?;

    let items = request::items_for_project(config, &project_id)?;

    let mut buffer = String::new();
    buffer.push_str(&green_string(&format!("Tasks for {project_name}")));

    for item in items::sort_by_datetime(items, config) {
        buffer.push('\n');
        buffer.push_str(&item.fmt(config));
    }
    Ok(buffer)
}

/// Empty the inbox by sending items to other projects one at a time
pub fn sort_inbox(config: Config) -> Result<String, String> {
    let inbox_id = projects::project_id(&config, "inbox")?;

    let items = request::items_for_project(&config, &inbox_id)?;

    if items.is_empty() {
        Ok(green_string("No tasks to sort in inbox"))
    } else {
        projects::list(config.clone())?;
        for item in items.iter() {
            move_item_to_project(config.clone(), item.to_owned())?;
        }
        Ok(green_string("Successfully sorted inbox"))
    }
}

/// Prioritize all unprioritized items in a project
pub fn prioritize_items(config: &Config, project_name: &str) -> Result<String, String> {
    let inbox_id = projects::project_id(config, project_name)?;

    let items = request::items_for_project(config, &inbox_id)?;

    let unprioritized_items: Vec<Item> = items
        .into_iter()
        .filter(|item| item.priority == 1)
        .collect::<Vec<Item>>();

    if unprioritized_items.is_empty() {
        Ok(format!("No tasks to prioritize in {project_name}")
            .green()
            .to_string())
    } else {
        for item in unprioritized_items.iter() {
            items::set_priority(config.clone(), item.to_owned());
        }
        Ok(format!("Successfully prioritized {project_name}")
            .green()
            .to_string())
    }
}

/// Put dates on all items without dates
pub fn date_items(config: &Config, project_name: &str) -> Result<String, String> {
    let project_id = projects::project_id(config, project_name)?;

    let items = request::items_for_project(config, &project_id)?;

    let undated_items: Vec<Item> = items
        .into_iter()
        .filter(|item| item.has_no_date() || item.is_overdue(config))
        .collect::<Vec<Item>>();

    if undated_items.is_empty() {
        Ok(format!("No tasks to date in {project_name}")
            .green()
            .to_string())
    } else {
        for item in undated_items.iter() {
            println!("{}", item.fmt(config));
            let due_string = config::get_input("Input a date in natural language or (c)omplete")?;
            match due_string.as_str() {
                "complete" | "c" => {
                    let config = config.set_next_id(item.id.clone());
                    request::complete_item(config)?
                }
                _ => request::update_item_due(config.clone(), item.to_owned(), due_string)?,
            };
        }
        Ok(format!("Successfully dated {project_name}")
            .green()
            .to_string())
    }
}
pub fn move_item_to_project(config: Config, item: Item) -> Result<String, String> {
    println!("{}", item.fmt(&config));

    let mut options = config
        .projects
        .keys()
        .map(|k| k.to_owned())
        .collect::<Vec<String>>();

    options.push("complete".to_string());
    options.reverse();

    let project_name =
        config::select_input("Enter destination project name or complete:", options)?;

    match project_name.as_str() {
        "complete" => {
            request::complete_item(config.set_next_id(item.id))?;
            Ok(green_string("✓"))
        }
        _ => {
            let project_id = projects::project_id(&config, &project_name)?;
            let sections = request::sections_for_project(&config, &project_id)?;
            let section_names: Vec<String> = sections.clone().into_iter().map(|x| x.name).collect();
            if section_names.is_empty() {
                request::move_item_to_project(config, item, &project_name)
            } else {
                let section_name = config::select_input("Select section", section_names)?;
                let section_id = &sections
                    .iter()
                    .find(|x| x.name == section_name.as_str())
                    .expect("Section does not exist")
                    .id;
                request::move_item_to_section(config, item, section_id)
            }
        }
    }
}

/// Add item to project with natural language processing
pub fn add_item_to_project(config: Config, task: &str, project: &str) -> Result<String, String> {
    let item = request::add_item_to_inbox(&config, task)?;

    match project {
        "inbox" | "i" => Ok(green_string("✓")),
        project => {
            request::move_item_to_project(config, item, project)?;
            Ok(green_string("✓"))
        }
    }
}

fn green_string(str: &str) -> String {
    String::from(str).green().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use mockito;
    use pretty_assertions::assert_eq;

    /// Need to adjust this value forward or back an hourwhen timezone changes
    const TIME: &str = "16:59";

    #[test]
    fn should_add_and_remove_projects() {
        let config = Config::new("123123", None).unwrap();
        let config = Config {
            path: "tests/project_test_config".to_string(),
            ..config
        };

        config.clone().create().unwrap();

        let params = vec![String::from("cool_project"), String::from("1")];
        let result = add(config.clone(), params);
        assert_eq!(result, Ok("✓".to_string()));

        let result = remove(config, "cool_project");
        assert_eq!(Ok("✓".to_string()), result);
    }
    #[test]
    fn should_list_projects() {
        let config = Config::new("123123", None)
            .unwrap()
            .add_project(String::from("first"), 1)
            .add_project(String::from("second"), 2);

        let str = if test::helpers::supports_coloured_output() {
            "\u{1b}[32mProjects\u{1b}[0m\n - first\n - second"
        } else {
            "Projects\n - first\n - second"
        };

        assert_eq!(list(config), Ok(String::from(str)));
    }

    #[test]
    fn should_get_next_item() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/sync/v9/projects/get_data")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&test::responses::items())
            .create();

        let config = Config::new("12341234", Some(server.url()))
            .unwrap()
            .add_project(String::from("good"), 1);

        let config_dir = dirs::config_dir().unwrap().to_str().unwrap().to_owned();

        let config_with_timezone = Config {
            timezone: Some(String::from("US/Pacific")),
            path: format!("{config_dir}/test2"),
            mock_url: Some(server.url()),
            ..config.clone()
        };

        config_with_timezone.clone().create().unwrap();

        let string = if test::helpers::supports_coloured_output() {
            format!("\n\u{1b}[33mPut out recycling\u{1b}[0m\nDue: {TIME} ↻")
        } else {
            format!("\nPut out recycling\nDue: {TIME} ↻")
        };

        assert_eq!(
            next_item(config_with_timezone, "good"),
            Ok(String::from(string))
        );
    }

    #[test]
    fn should_display_scheduled_items() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/sync/v9/projects/get_data")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&test::responses::items())
            .create();

        let config = Config::new("12341234", Some(server.url()))
            .unwrap()
            .add_project(String::from("good"), 1);

        let config_with_timezone = Config {
            timezone: Some(String::from("US/Pacific")),
            mock_url: Some(server.url()),
            ..config
        };

        assert_eq!(
            scheduled_items(&config_with_timezone, "test"),
            Err(String::from(
                "Project test not found, please add it to config"
            ))
        );

        let string = if test::helpers::supports_coloured_output() {
            format!("\u{1b}[32mSchedule for good\u{1b}[0m\n\n\u{1b}[33mPut out recycling\u{1b}[0m\nDue: {TIME} ↻")
        } else {
            format!("Schedule for good\n\nPut out recycling\nDue: {TIME} ↻")
        };
        let result = scheduled_items(&config_with_timezone, "good");
        assert_eq!(result, Ok(string));
    }

    #[test]
    fn should_list_all_items() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/sync/v9/projects/get_data")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&test::responses::items())
            .create();

        let config = Config::new("12341234", Some(server.url()))
            .unwrap()
            .add_project(String::from("good"), 1);

        let config_with_timezone = Config {
            timezone: Some(String::from("US/Pacific")),
            mock_url: Some(server.url()),
            ..config
        };

        let string = if test::helpers::supports_coloured_output() {
            format!("\u{1b}[32mTasks for good\u{1b}[0m\n\n\u{1b}[33mPut out recycling\u{1b}[0m\nDue: {TIME} ↻")
        } else {
            format!("Tasks for good\n\nPut out recycling\nDue: {TIME} ↻")
        };
        assert_eq!(all_items(&config_with_timezone, "good"), Ok(string));
        mock.assert();
    }
}
