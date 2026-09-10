#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    String,
    Enum,
    Number,
    Boolean,
    Timestamp,
    Json,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryField {
    pub name: String,
    pub field_type: FieldType,
    pub enum_values: Option<Vec<String>>,
}

impl QueryField {
    pub fn new(name: &str, field_type: FieldType) -> Self {
        Self {
            name: name.to_string(),
            field_type,
            enum_values: None,
        }
    }

    pub fn with_enum_values(mut self, values: Vec<String>) -> Self {
        self.enum_values = Some(values);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogSchema {
    pub fields: Vec<QueryField>,
}

impl LogSchema {
    pub fn find_field(&self, name: &str) -> Option<&QueryField> {
        self.fields.iter().find(|f| f.name == name)
    }
}
