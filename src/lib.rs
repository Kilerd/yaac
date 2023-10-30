use itertools::{EitherOrBoth, Itertools};
use regex::{Captures, Regex};
use serde::Deserialize;
use std::cmp::max;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use toml::{Table, Value};
use crate::source::Source;
use crate::utils::build_toml_value;


mod source;

pub mod utils;

pub struct ConfigLoader {
    sources: Vec<Box<dyn Source + Send + Sync>>
}

impl ConfigLoader {
    pub fn new() -> Self {
        ConfigLoader {
            sources: vec![]
        }
    }
    pub fn add_source<T: Source  + Send + Sync + 'static>(&mut self, source: T) {
        self.sources.push(Box::new(source))
    }

    pub fn construct<'de, T: Deserialize<'de>>(self) -> Result<T, Box<dyn std::error::Error>> {

        let mut source_ret = vec![];
        for source in self.sources {
            let value1 = source.load()?;
            source_ret.push(value1);
        }

        let mut result = Value::Table(Table::new());

        // merge all values
        for environment_value in source_ret {
            result = merge_two_value(result, environment_value, "$")?;
        }

        // resolve placeholder
        resolve_environment_placeholder(&mut result);
        let mut collectors = HashMap::new();
        collect_path_placeholder(&result, &result, &mut collectors);
        resolve_path_placeholder(&mut result, &collectors);
        dbg!(&collectors);
        Ok(result.try_into()?)
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


#[derive(Debug, PartialEq)]
pub struct Error {
    pub path: String,
    pub existed_type: &'static str,
    pub appended_type: &'static str,
}

impl Error {
    pub fn new(path: String, existed_type: &'static str, appended_type: &'static str) -> Self {
        Self {
            path,
            existed_type,
            appended_type,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "merge fail, path={}, existed type={} appended type={}",
            self.path, self.existed_type, self.appended_type
        )
    }
}

impl std::error::Error for Error {}

fn merge_into_table_inner(value: &mut Table, other: Table, path: &str) -> Result<(), Error> {
    for (name, inner) in other {
        if let Some(existing) = value.remove(&name) {
            let inner_path = format!("{path}.{name}");
            value.insert(name, merge_two_value(existing, inner, &inner_path)?);
        } else {
            value.insert(name, inner);
        }
    }
    Ok(())
}

fn merge_two_value(base: Value, append: Value, path: &str) -> Result<Value, Error> {
    match (base, append) {
        (Value::String(_), Value::String(inner)) => Ok(Value::String(inner)),
        (Value::Integer(_), Value::Integer(inner)) => Ok(Value::Integer(inner)),
        (Value::Float(_), Value::Float(inner)) => Ok(Value::Float(inner)),
        (Value::Boolean(_), Value::Boolean(inner)) => Ok(Value::Boolean(inner)),
        (Value::Datetime(_), Value::Datetime(inner)) => Ok(Value::Datetime(inner)),
        (Value::Array(existing), Value::Array(inner)) => {
            let mut ret = Vec::with_capacity(max(existing.len(), inner.len()));
            for pair in existing
                .into_iter()
                .enumerate()
                .zip_longest(inner.into_iter().enumerate())
            {
                let element = match pair {
                    EitherOrBoth::Both(l, r) => {
                        merge_two_value(l.1, r.1, &format!("{}.[{}]", path, l.0))?
                    }
                    EitherOrBoth::Left(l) => l.1,
                    EitherOrBoth::Right(r) => r.1,
                };
                ret.push(element);
            }
            Ok(Value::Array(ret))
        }
        (Value::Table(mut existing), Value::Table(inner)) => {
            merge_into_table_inner(&mut existing, inner, path)?;
            Ok(Value::Table(existing))
        }
        (v, o) => Err(Error::new(path.to_owned(), v.type_str(), o.type_str())),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    use crate::ConfigLoader;
    use crate::source::environment::EnvironmentSource;
    use crate::source::file_based::FileSource;

    #[test]
    fn should_load_config_from_toml_file() {
        #[derive(Debug, Deserialize)]
        struct Config {
            a: i32,
        }
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("application.toml");
        let mut tmp_file = File::create(&file_path).unwrap();
        writeln!(tmp_file, "a = 123").unwrap();

        let mut loader = ConfigLoader::new();
        loader.add_source(FileSource::new(file_path));

        let config: Config = loader.construct().unwrap();

        assert_eq!(123, config.a)
    }

    #[test]
    fn should_load_config_from_given_environment() {
        #[derive(Debug, Deserialize)]
        struct Config {
            value: String,
        }

        temp_env::with_var("APP_VALUE", Some("value here"), || {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("application.toml");
            let mut tmp_file = File::create(&file_path).unwrap();
            writeln!(tmp_file, "a = 123").unwrap();


            let mut loader = ConfigLoader::new();
            loader.add_source(FileSource::new(file_path));
            loader.add_source(EnvironmentSource::new("APP"));

            let config: Config = loader.construct().unwrap();

            assert_eq!("value here", config.value);
        });
    }

    #[test]
    fn should_load_config_from_given_environment_with_prefix() {
        #[derive(Debug, Deserialize)]
        struct Config {
            value: String,
        }

        temp_env::with_var("YAAC_VALUE", Some("value here"), || {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("application.toml");
            let mut tmp_file = File::create(&file_path).unwrap();
            writeln!(tmp_file, "a = 123").unwrap();

            let mut loader = ConfigLoader::new();
            loader.add_source(FileSource::new(file_path));
            loader.add_source(EnvironmentSource::new("YAAC"));

            let config: Config = loader.construct().unwrap();

            assert_eq!("value here", config.value);
        });
    }

    #[test]
    fn should_override_config_from_given_environment() {
        #[derive(Debug, Deserialize)]
        struct Config {
            value: String,
        }

        temp_env::with_var("YAAC_VALUE", Some("value here"), || {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("application.toml");
            let mut tmp_file = File::create(&file_path).unwrap();
            writeln!(tmp_file, "value = \"original\"").unwrap();

            let mut loader = ConfigLoader::new();
            loader.add_source(FileSource::new(file_path));
            loader.add_source(EnvironmentSource::new("YAAC"));

            let config: Config = loader.construct().unwrap();
            assert_eq!("value here", config.value);
        });
    }

    #[test]
    fn should_resolve_placeholder() {
        #[derive(Debug, Deserialize)]
        struct Config {
            hello_key: String,
            all: String,
        }

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("application.toml");
        let mut tmp_file = File::create(&file_path).unwrap();
        writeln!(
            tmp_file,
            "hello_key = \"hello\" \nall=\"${{hello_key}} world\""
        )
        .unwrap();


        let mut loader = ConfigLoader::new();
        loader.add_source(FileSource::new(file_path));
        loader.add_source(EnvironmentSource::new("APP"));

        let config: Config = loader.construct().unwrap();
        assert_eq!("hello", config.hello_key);
        assert_eq!("hello world", config.all);
    }

    #[test]
    fn should_resolve_environment_placeholder() {
        #[derive(Debug, Deserialize)]
        struct Config {
            value: String,
        }

        temp_env::with_var("YAAC_VALUE", Some("value here"), || {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("application.toml");
            let mut tmp_file = File::create(&file_path).unwrap();
            writeln!(tmp_file, "value = \"more ${{YAAC_VALUE}}\"").unwrap();

            let mut loader = ConfigLoader::new();
            loader.add_source(FileSource::new(file_path));
            loader.add_source(EnvironmentSource::new("APP"));

            let config: Config = loader.construct().unwrap();
            assert_eq!("more value here", config.value);
        });
    }

    #[test]
    fn should_load_config_from_hierarchy_files() {}
}
