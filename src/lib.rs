use std::cmp::{max};
use std::fmt::{Display, format, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use itertools::{EitherOrBoth, Itertools, Position};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use toml::{Table, Value};

pub struct ConfigLoader {
    file_base_name: String,
    environment_prefix: String,
}

impl ConfigLoader {
    pub fn new(file_base_name: impl Into<String>, environment_prefix: impl Into<String>) -> Self {
        ConfigLoader {
            file_base_name: file_base_name.into(),
            environment_prefix: environment_prefix.into(),
        }
    }

    pub fn construct<'de, T: Deserialize<'de>>(self) -> Result<T, Box<dyn std::error::Error>> {
        // load from file
        let profile: Option<String> = None;
        let profile_string = profile.map(|it| format!("_{}", it)).unwrap_or("".to_string());
        let buf = PathBuf::from_str(&format!("{}{}.toml", &self.file_base_name, profile_string))?;
        let file_content = std::fs::read_to_string(buf)?;
        let mut result: toml::Value = toml::from_str(&file_content)?;

        // load from env
        let prefix = format!("{}_", &self.environment_prefix);
        let environments: Vec<toml::Value> = std::env::vars().into_iter().filter(|(key, value)| key.starts_with(&prefix))
            .map(|(key, value)| (key.strip_prefix(&prefix).unwrap_or(&key).to_owned(), value))
            .filter(|(key, value)| !key.is_empty())
            .map(|(key, value)| build_toml_value(key.to_lowercase(), value))
            .collect();

        // merge all values
        for environment_value in environments {
            result = merge_two_value(result, environment_value, "$")?;
        }

        Ok(result.try_into()?)
    }
}

fn build_toml_value(key: String, value: String) -> toml::Value {
    let split = key.split("_").into_iter().collect_vec();

    let mut rev = split.into_iter().rev();
    let first = rev.next().unwrap();
    let value1 = Value::String(value.to_owned());
    let mut accr = Table::new();
    accr.insert(first.to_owned(), value1);
    let accr = Value::Table(accr);
    let ret = rev.fold(accr, |accr, text| {
        let mut map = Table::new();
        map.insert(text.to_owned(), accr);
        Value::Table(map)
    });
    ret
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
        write!(f, "merge fail, path={}, existed type={} appended type={}", self.path, self.existed_type, self.appended_type)
    }
}

impl std::error::Error for Error {

}


fn merge_into_table_inner(
    value: &mut Table,
    other: Table,
    path: &str,
) -> Result<(), Error> {
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
    match(base, append) {
        (Value::String(_), Value::String(inner)) => Ok(Value::String(inner)),
        (Value::Integer(_), Value::Integer(inner)) => Ok(Value::Integer(inner)),
        (Value::Float(_), Value::Float(inner)) => Ok(Value::Float(inner)),
        (Value::Boolean(_), Value::Boolean(inner)) => Ok(Value::Boolean(inner)),
        (Value::Datetime(_), Value::Datetime(inner)) => Ok(Value::Datetime(inner)),
        (Value::Array(existing), Value::Array(inner)) => {
            let mut ret = Vec::with_capacity(max(existing.len(), inner.len()));
            for pair in existing.into_iter().enumerate().zip_longest(inner.into_iter().enumerate()) {
               let element =  match pair {
                    EitherOrBoth::Both(l, r) => merge_two_value(l.1, r.1, &format!("{}.[{}]", path, l.0))?,
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
    use std::fs::File;
    use tempfile::tempdir;
    use std::io::{self, Write};
    use serde::Deserialize;

    use crate::ConfigLoader;

    #[test]
    fn should_load_config_from_toml_file() {
        #[derive(Debug, Deserialize)]
        struct Config {
            a: i32,
        }
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("application.toml");
        let mut tmp_file = File::create(file_path).unwrap();
        writeln!(tmp_file, "a = 123").unwrap();

        let buf = dir.path().join("application").to_str().unwrap().to_string();
        let config_loader = ConfigLoader::new(buf, "APP");
        let config: Config = config_loader.construct().unwrap();

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
            let mut tmp_file = File::create(file_path).unwrap();
            writeln!(tmp_file, "a = 123").unwrap();

            let buf = dir.path().join("application").to_str().unwrap().to_string();
            let config_loader = ConfigLoader::new(buf, "APP");
            let config: Config = config_loader.construct().unwrap();

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
            let mut tmp_file = File::create(file_path).unwrap();
            writeln!(tmp_file, "a = 123").unwrap();

            let buf = dir.path().join("application").to_str().unwrap().to_string();
            let config_loader = ConfigLoader::new(buf, "YAAC");
            let config: Config = config_loader.construct().unwrap();

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
            let mut tmp_file = File::create(file_path).unwrap();
            writeln!(tmp_file, "value = \"original\"").unwrap();

            let buf = dir.path().join("application").to_str().unwrap().to_string();
            let config_loader = ConfigLoader::new(buf, "YAAC");
            let config: Config = config_loader.construct().unwrap();

            assert_eq!("value here", config.value);
        });
    }


    #[test]
    fn should_load_config_from_hierarchy_files() {}
}
