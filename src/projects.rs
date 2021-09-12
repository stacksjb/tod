use crate::config::Config;
use crate::items::Item;
use crate::{items, projects, request};

const ADD_ERROR: &str = "Must provide project name and number, i.e. tod --add projectname 12345";

/// List the projects in config
pub fn list(config: Config) {
    println!("== Projects ==");
    for (k, _) in config.projects.iter() {
        println!("{}", k);
    }
}

/// Add a project to the projects HashMap in Config
pub fn add(config: Config, params: Vec<&str>) {
    let mut params = params.clone();
    let num = params
        .pop()
        .expect(ADD_ERROR)
        .parse::<u32>()
        .expect(ADD_ERROR);

    let name = params.pop().expect(ADD_ERROR);

    config.add_project(name, num).save()
}

/// Remove a project from the projects HashMap in Config
pub fn remove(config: Config, project_name: &str) {
    config.remove_project(project_name).save()
}

pub fn project_id(config: &Config, project_name: &str) -> String {
    let project_id = *config.projects.get(project_name).unwrap_or_else(|| {
        panic!(
            "Project {} not found, please add it to config",
            project_name
        )
    });

    project_id.to_string()
}

/// Get the next item by priority
pub fn next_item(config: Config, project_name: &str) {
    let project_id = projects::project_id(&config, project_name);

    match request::items_for_project(config.clone(), &project_id) {
        Ok(items) => {
            let maybe_item = items::sort_by_priority(items)
                .first()
                .map(|item| item.to_owned());

            match maybe_item {
                Some(item) => {
                    config.set_next_id(item.id).save();
                    println!("{}", item);
                }
                None => print!("No items on list"),
            }
        }
        Err(e) => println!("{}", e),
    }
}

/// Sort all the items in inbox
pub fn sort_inbox(config: Config) {
    let inbox_id = projects::project_id(&config, "inbox");

    match request::items_for_project(config.clone(), &inbox_id) {
        Ok(items) if !items.is_empty() => {
            projects::list(config.clone());
            for item in items.iter() {
                request::move_item_to_project(config.clone(), item.to_owned());
            }
        }
        Ok(_) => println!("No tasks to sort in inbox"),
        Err(e) => println!("{}", e),
    }
}

/// Prioritize all items in a project
pub fn prioritize_items(config: Config, project_name: &str) {
    let inbox_id = projects::project_id(&config, project_name);

    match request::items_for_project(config.clone(), &inbox_id) {
        Ok(items) => {
            let unprioritized_items: Vec<Item> = items
                .into_iter()
                .filter(|item| item.priority == 1)
                .collect::<Vec<Item>>();

            if unprioritized_items.is_empty() {
                println!("No tasks to prioritize in {}", project_name)
            } else {
                projects::list(config.clone());
                for item in unprioritized_items.iter() {
                    items::set_priority(config.clone(), item.to_owned());
                }
            }
        }

        Err(e) => println!("{}", e),
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::config;
//     use std::collections::HashMap;

//     #[test]
//     fn add_and_remove_project_should_work() {
//         // Add a project
//         let config = Config::new("abcd");
//         let params = vec!["some_project", "1234"];

//         let mut projects: HashMap<String, u32> = HashMap::new();
//         projects.insert(String::from("some_project"), 1234);
//         let new_config = config::Config {
//             path: config::generate_path(),
//             token: String::from("abcd"),
//             next_id: None,
//             projects: projects.clone(),
//         };

//         let config_with_one_project = add(config, params);

//         assert_eq!(config_with_one_project, new_config);

//         // Add a second project
//         projects.insert(String::from("some_other_project"), 2345);
//         let params = vec!["some_other_project", "3456"];

//         let config_with_two_projects = add(config_with_one_project, params);

//         // Remove the first project
//         let config_with_other_project = remove(config_with_two_projects, "some_project");

//         let mut projects: HashMap<String, u32> = HashMap::new();
//         projects.insert(String::from("some_other_project"), 3456);
//         let new_config = config::Config {
//             path: config::generate_path(),
//             token: String::from("abcd"),
//             next_id: None,
//             projects,
//         };

//         assert_eq!(config_with_other_project, new_config);
//     }
// }
