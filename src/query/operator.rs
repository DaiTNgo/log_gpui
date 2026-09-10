#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryOperator {
    Eq,
    Neq,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
    In,
}

impl QueryOperator {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryOperator::Eq => "==",
            QueryOperator::Neq => "!=",
            QueryOperator::Contains => "contains",
            QueryOperator::StartsWith => "startsWith",
            QueryOperator::EndsWith => "endsWith",
            QueryOperator::Matches => "matches",
            QueryOperator::In => "in",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            QueryOperator::Eq => "equals",
            QueryOperator::Neq => "does not equal",
            QueryOperator::Contains => "contains substring",
            QueryOperator::StartsWith => "starts with",
            QueryOperator::EndsWith => "ends with",
            QueryOperator::Matches => "matches regex",
            QueryOperator::In => "is one of",
        }
    }
}

pub fn operators_for_type(field_type: crate::query::schema::FieldType) -> Vec<QueryOperator> {
    use crate::query::schema::FieldType;
    match field_type {
        FieldType::String | FieldType::Timestamp | FieldType::Json => vec![
            QueryOperator::Eq,
            QueryOperator::Neq,
            QueryOperator::Contains,
            QueryOperator::StartsWith,
            QueryOperator::EndsWith,
            QueryOperator::Matches,
        ],
        FieldType::Enum => vec![
            QueryOperator::Eq,
            QueryOperator::Neq,
            QueryOperator::In,
        ],
        FieldType::Number => vec![
            QueryOperator::Eq,
            QueryOperator::Neq,
        ],
        FieldType::Boolean => vec![
            QueryOperator::Eq,
            QueryOperator::Neq,
        ],
    }
}
