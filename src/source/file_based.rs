use std::error::Error;
use std::path::PathBuf;
use toml::Value;
use crate::source::Source;

pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}


impl Source for FileSource {
    fn load(&self) -> Result<Value, Box<dyn Error>> {
        let file_content = std::fs::read_to_string(&self.path)?;
        let result: Value = toml::from_str(&file_content)?;

        Ok(result)
    }
}
