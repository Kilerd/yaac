use crate::merge_two_value;
use crate::source::Source;
use crate::utils::build_toml_value;
use std::error::Error;
use toml::{Table, Value};

pub struct EnvironmentSource {
    prefix: String,
}

impl EnvironmentSource {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl Source for EnvironmentSource {
    fn load(&self) -> Result<Value, Box<dyn Error>> {
        let prefix = format!("{}_", &self.prefix);
        let environments: Vec<Value> = std::env::vars()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, value)| (key.strip_prefix(&prefix).unwrap_or(&key).to_owned(), value))
            .filter(|(key, _)| !key.is_empty())
            .map(|(key, value)| build_toml_value(key.to_lowercase(), value))
            .collect();

        let mut accr = Value::Table(Table::new());
        for x in environments.into_iter() {
            accr = merge_two_value(accr, x, "$")?;
        }

        Ok(accr)
    }
}
