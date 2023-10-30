use std::path::PathBuf;
use std::str::FromStr;
use serde::de::DeserializeOwned;
use serde::Deserialize;

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
        dbg!(&self.file_base_name);
        let profile : Option<String>= None;
        let profile_string = profile.map(|it| format!("_{}", it)).unwrap_or("".to_string());
        let buf = PathBuf::from_str(&format!("{}{}.toml", &self.file_base_name, profile_string))?;
        let file_content = std::fs::read_to_string(buf)?;
        let result:toml::Value = toml::from_str(&file_content)?;
        Ok(result.try_into()?)
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
            a: i32
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
    fn should_load_config_from_given_environment() {}

    #[test]
    fn should_load_config_from_hierachy_files() {}
}
