use itertools::Itertools;
use toml::{Value,Table};

pub fn build_toml_value(key: String, value: String) -> toml::Value {
    let split = key.split('_').collect_vec();

    let mut rev = split.into_iter().rev();
    let first = rev.next().unwrap();
    let value1 = Value::String(value.to_owned());
    let mut accr = Table::new();
    accr.insert(first.to_owned(), value1);
    let accr = Value::Table(accr);
    rev.fold(accr, |accr, text| {
        let mut map = Table::new();
        map.insert(text.to_owned(), accr);
        Value::Table(map)
    })
}