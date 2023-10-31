use itertools::Itertools;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::error::Error;
use toml::Value;

pub trait Processor {
    fn process(&self, value: &mut Value) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct EnvironmentVariableProcessor;

impl Processor for EnvironmentVariableProcessor {
    fn process(&self, value: &mut Value) -> Result<(), Box<dyn Error>> {
        fn resolve_environment_placeholder(value: &mut Value) {
            let environment_pattern = Regex::new("\\$\\{(?<env>[A-Z]+(_[A-Z]+)*)\\}").unwrap();

            match value {
                Value::String(ref mut inner) => {
                    let ret = environment_pattern.replace_all(inner, |caps: &Captures| {
                        let env_variable: &str = &caps["env"];
                        std::env::var(env_variable).unwrap_or("".to_owned())
                    });
                    *inner = ret.to_string();
                }
                Value::Array(inner) => {
                    for element in inner {
                        resolve_environment_placeholder(element);
                    }
                }
                Value::Table(table) => {
                    for (_, value) in table.iter_mut() {
                        resolve_environment_placeholder(value);
                    }
                }
                _ => {}
            }
        }

        resolve_environment_placeholder(value);
        Ok(())
    }
}

pub struct PathVariableProcessor;

impl Processor for PathVariableProcessor {
    fn process(&self, value: &mut Value) -> Result<(), Box<dyn Error>> {
        let mut collectors = HashMap::new();
        collect_path_placeholder(value, value, &mut collectors);
        resolve_path_placeholder(value, &collectors);
        Ok(())
    }
}

fn get_value_by_path_inner<'b, 'a: 'b>(
    value: &'a Value,
    paths: &'b [&'b str],
) -> Option<&'a Value> {
    match paths.len() {
        0 => unreachable!(),
        1 => value.as_table().and_then(|table| table.get(paths[0])),
        _ => {
            let option = value.as_table().and_then(|table| table.get(paths[0]));
            option.and_then(|tier| get_value_by_path_inner(tier, &paths[1..]))
        }
    }
}

fn get_value_by_path<'b, 'a: 'b>(value: &'a Value, path: &'b str) -> Option<&'a Value> {
    let paths = path.split('.').collect_vec();
    get_value_by_path_inner(value, &paths[..])
}

fn resolve_path_placeholder(value: &mut Value, collectors: &HashMap<String, String>) {
    let path_pattern =
        Regex::new("\\$\\{(?<path>[a-z]+(_[a-z]+)*(\\.[a-z]+(_[a-z]+)*)*)\\}").unwrap();

    match value {
        Value::String(ref mut inner) => {
            let ret = path_pattern.replace_all(inner, |caps: &Captures| {
                let path: &str = &caps["path"];
                collectors.get(path).cloned().unwrap_or("".to_string())
            });
            *inner = ret.to_string();
        }
        Value::Array(inner) => {
            for element in inner {
                resolve_path_placeholder(element, collectors);
            }
        }
        Value::Table(table) => {
            for (_, value) in table.iter_mut() {
                resolve_path_placeholder(value, collectors);
            }
        }
        _ => {}
    }
}

fn collect_path_placeholder(root: &Value, value: &Value, collectors: &mut HashMap<String, String>) {
    let path_pattern =
        Regex::new("\\$\\{(?<path>[a-z]+(_[a-z]+)*(\\.[a-z]+(_[a-z]+)*)*)\\}").unwrap();

    match value {
        Value::String(inner) => {
            for caps in path_pattern.captures_iter(inner) {
                let path: &str = &caps["path"];
                collectors.insert(
                    path.to_string(),
                    get_value_by_path(root, path)
                        .and_then(|it| it.as_str())
                        .map(|it| it.to_string())
                        .unwrap_or("".to_string()),
                );
            }
        }
        Value::Array(inner) => {
            for element in inner {
                collect_path_placeholder(root, element, collectors);
            }
        }
        Value::Table(table) => {
            for (_, value) in table.iter() {
                collect_path_placeholder(root, value, collectors);
            }
        }
        _ => {}
    }
}
