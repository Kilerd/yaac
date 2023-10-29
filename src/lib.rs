use serde::de::DeserializeOwned;

pub struct Config {
    file_base_name: String,
    environment_prefix: String,
}

impl Config {
    pub fn new(file_base_name: impl Into<String>, environment_prefix: impl Into<String>) -> Self {
        Config {
            file_base_name: file_base_name.into(),
            environment_prefix: environment_prefix.into(),
        }
    }

    pub fn construct<T: DeserializeOwned>(self) -> Result<T, Box<dyn std::error::Error>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use tempfile::tempdir;
    use std::io::{self, Write};

    use crate::Config;

    #[test]
    fn should_load_config_from_toml_file() {
        
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("application.toml");
        let mut tmp_file = File::create(file_path).unwrap();
        writeln!(tmp_file, "a = 123").unwrap();
        
        dir.path().join("application").
        let config = Config::new("application", "APP");
    }

    #[test]
    fn should_load_config_from_given_environment() {}

    #[test]
    fn should_load_config_from_hierachy_files() {}
}
