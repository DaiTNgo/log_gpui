use crate::query::ast::{QueryCondition, QueryExpression, QueryGroup};
use crate::query::diagnostic::Diagnostic;
use crate::query::operator::operators_for_type;
use crate::query::schema::{FieldType, LogSchema};
use crate::query::value::QueryValue;

pub fn validate(query: &QueryExpression, schema: &LogSchema) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    validate_expr(query, schema, &mut diagnostics);
    diagnostics
}

fn validate_expr(
    expr: &QueryExpression,
    schema: &LogSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        QueryExpression::Condition(cond) => validate_condition(cond, schema, diagnostics),
        QueryExpression::Group(group) => validate_group(group, schema, diagnostics),
    }
}

fn validate_group(
    group: &QueryGroup,
    schema: &LogSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for child in &group.children {
        validate_expr(child, schema, diagnostics);
    }
}

fn validate_condition(
    cond: &QueryCondition,
    schema: &LogSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(field) = schema.find_field(&cond.field) else {
        diagnostics.push(Diagnostic::error(
            format!("unknown field '{}'", cond.field),
            cond.field_span.clone(),
        ));
        return;
    };

    let valid_ops = operators_for_type(field.field_type);
    if !valid_ops.contains(&cond.operator) {
        diagnostics.push(Diagnostic::error(
            format!(
                "operator '{}' is not valid for field '{}' of type {:?}",
                cond.operator.as_str(),
                cond.field,
                field.field_type
            ),
            cond.operator_span.clone(),
        ));
    }

    // Type check value against field type
    if !value_matches_type(&cond.value, field.field_type) {
        diagnostics.push(Diagnostic::error(
            format!("value does not match field '{}' of type {:?}", cond.field, field.field_type),
            cond.value_span.clone(),
        ));
    }

    // Enum value check
    if let Some(values) = &field.enum_values {
        let value_str = cond.value.to_string_value();
        if !values.iter().any(|v| v.eq_ignore_ascii_case(&value_str)) {
            diagnostics.push(Diagnostic::error(
                format!(
                    "invalid value '{}' for field '{}'; expected one of: {}",
                    value_str,
                    cond.field,
                    values.join(", ")
                ),
                cond.value_span.clone(),
            ));
        }
    }
}

fn value_matches_type(value: &QueryValue, field_type: FieldType) -> bool {
    match (value, field_type) {
        (QueryValue::String(_), FieldType::String | FieldType::Timestamp | FieldType::Json | FieldType::Enum) => true,
        (QueryValue::Number(_), FieldType::Number) => true,
        (QueryValue::Boolean(_), FieldType::Boolean) => true,
        _ => false,
    }
}