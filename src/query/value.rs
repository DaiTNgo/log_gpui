#[derive(Debug, Clone, PartialEq)]
pub enum QueryValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl QueryValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            QueryValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            QueryValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            QueryValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn to_string_value(&self) -> String {
        match self {
            QueryValue::String(s) => s.clone(),
            QueryValue::Number(n) => n.to_string(),
            QueryValue::Boolean(b) => b.to_string(),
        }
    }
}
