#[derive(Debug, Clone, PartialEq)]
pub enum QueryExpression {
    Condition(QueryCondition),
    Group(QueryGroup),
}

impl QueryExpression {
    pub fn condition(cond: QueryCondition) -> Self {
        QueryExpression::Condition(cond)
    }

    pub fn group(operator: GroupOperator, children: Vec<QueryExpression>) -> Self {
        QueryExpression::Group(QueryGroup::new(operator, children))
    }

    pub fn is_condition(&self) -> bool {
        matches!(self, QueryExpression::Condition(_))
    }

    pub fn is_group(&self) -> bool {
        matches!(self, QueryExpression::Group(_))
    }

    pub fn as_condition(&self) -> Option<&QueryCondition> {
        match self {
            QueryExpression::Condition(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_group(&self) -> Option<&QueryGroup> {
        match self {
            QueryExpression::Group(g) => Some(g),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryCondition {
    pub field: String,
    pub operator: crate::query::operator::QueryOperator,
    pub value: crate::query::value::QueryValue,
    /// Byte span of the field name in the source text
    pub field_span: std::ops::Range<usize>,
    /// Byte span of the operator token in the source text
    pub operator_span: std::ops::Range<usize>,
    /// Byte span of the value in the source text
    pub value_span: std::ops::Range<usize>,
}

impl QueryCondition {
    pub fn new(
        field: &str,
        operator: crate::query::operator::QueryOperator,
        value: crate::query::value::QueryValue,
    ) -> Self {
        Self {
            field: field.to_string(),
            operator,
            value,
            field_span: 0..0,
            operator_span: 0..0,
            value_span: 0..0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryGroup {
    pub operator: GroupOperator,
    pub children: Vec<QueryExpression>,
    /// Byte span covering the whole group in the source text
    pub span: std::ops::Range<usize>,
}

impl QueryGroup {
    pub fn new(operator: GroupOperator, children: Vec<QueryExpression>) -> Self {
        Self {
            operator,
            children,
            span: 0..0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupOperator {
    And,
    Or,
}