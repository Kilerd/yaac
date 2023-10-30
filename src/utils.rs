use std::fmt::{Display, Formatter};
use itertools::{EitherOrBoth, Itertools};
use toml::{Value,Table};
use std::cmp::max;

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

pub fn merge_two_value(base: Value, append: Value, path: &str) -> Result<Value, Error> {
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
