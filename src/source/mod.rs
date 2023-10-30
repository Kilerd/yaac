use toml::Value;


pub mod file_based;
pub mod environment;

pub trait Source{
    fn load(&self) -> Result<Value, Box<dyn std::error::Error>>;
}

