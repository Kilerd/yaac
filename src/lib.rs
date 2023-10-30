use crate::utils::merge_two_value;
use serde::Deserialize;
use toml::{Table, Value};

mod post_processor;
mod source;

pub mod utils;

pub use crate::post_processor::{
    EnvironmentPostProcessor, PathVariablePostProcessor, PostProcessor,
};
pub use crate::source::{environment::EnvironmentSource, file_based::FileSource, Source};

#[derive(Default)]
pub struct ConfigLoader {
    sources: Vec<Box<dyn Source + Send + Sync>>,
    post_processors: Vec<Box<dyn PostProcessor + Send + Sync>>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        ConfigLoader::default()
    }
    pub fn add_source<T: Source + Send + Sync + 'static>(&mut self, source: T) {
        self.sources.push(Box::new(source))
    }
    pub fn add_post_processor<T: PostProcessor + Send + Sync + 'static>(
        &mut self,
        post_processor: T,
    ) {
        self.post_processors.push(Box::new(post_processor))
    }

    pub fn enable_environment_post_processor(&mut self) {
        self.add_post_processor(EnvironmentPostProcessor);
    }
    pub fn enable_path_variable_post_processor(&mut self) {
        self.add_post_processor(PathVariablePostProcessor);
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

        for post_processor in self.post_processors {
            post_processor.process(&mut result)?;
        }

        Ok(result.try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    use crate::source::environment::EnvironmentSource;
    use crate::source::file_based::FileSource;
    use crate::ConfigLoader;

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
        loader.enable_path_variable_post_processor();

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
            loader.enable_environment_post_processor();

            let config: Config = loader.construct().unwrap();
            assert_eq!("more value here", config.value);
        });
    }

    #[test]
    fn should_load_config_from_hierarchy_files() {}
}
