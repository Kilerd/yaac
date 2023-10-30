use toml::Value;

pub mod environment;
pub mod file_based;

pub trait Source {
    fn load(&self) -> Result<Value, Box<dyn std::error::Error>>;
}
