//! Convert controller metadata and JSON Schema contracts into OpenAPI 3.1.

use crate::ApiOperation;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Invalid or conflicting route documentation discovered before serving traffic.
#[derive(Debug)]
pub struct DocumentationError(String);

impl std::fmt::Display for DocumentationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for DocumentationError {}

/// Generate a schema using the same Serde contract as the DTO or response entity.
pub fn json_schema<T: schemars::JsonSchema>() -> Value {
    schemars::schema_for!(T).to_value()
}

// Promote definitions to document components and rewrite references recursively.
fn normalize_schema(
    mut schema: Value,
    components: &mut Map<String, Value>,
) -> Result<Value, DocumentationError> {
    if let Some(object) = schema.as_object_mut() {
        object.remove("$schema");
        if let Some(Value::Object(definitions)) = object.remove("$defs") {
            for (name, definition) in definitions {
                let definition = normalize_schema(definition, components)?;
                if components
                    .get(&name)
                    .is_some_and(|existing| existing != &definition)
                {
                    return Err(DocumentationError(format!(
                        "conflicting schema name: {name}"
                    )));
                }
                components.insert(name, definition);
            }
        }
    }
    fn rewrite(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(reference)) = object.get_mut("$ref") {
                    if let Some(name) = reference.strip_prefix("#/$defs/") {
                        *reference = format!("#/components/schemas/{name}");
                    }
                }
                for value in object.values_mut() {
                    rewrite(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    rewrite(value);
                }
            }
            _ => {}
        }
    }
    rewrite(&mut schema);
    Ok(schema)
}

/// Build an OpenAPI 3.1 document; reject duplicate paths, IDs, and schema names.
pub fn openapi_document(
    title: &str,
    version: &str,
    operations: Vec<ApiOperation>,
) -> Result<Value, DocumentationError> {
    let mut paths = Map::new();
    let mut components = Map::new();
    let mut identifiers = HashSet::new();
    for operation in operations {
        if !identifiers.insert(operation.operation_id) {
            return Err(DocumentationError(format!(
                "duplicate operation ID: {}",
                operation.operation_id
            )));
        }
        let mut responses = Map::new();
        for response in operation.responses {
            let mut entry = json!({"description": response.description});
            if let Some(schema) = response.schema {
                entry["content"] = json!({"application/json": {"schema": normalize_schema(schema, &mut components)?}});
            }
            if responses
                .insert(response.status.to_string(), entry)
                .is_some()
            {
                return Err(DocumentationError(format!(
                    "duplicate response status: {}",
                    response.status
                )));
            }
        }
        if responses.is_empty() {
            responses.insert("default".into(), json!({"description": "Response"}));
        }
        let mut entry = json!({"summary": operation.summary, "operationId": operation.operation_id, "responses": responses});
        if !operation.tag.is_empty() {
            entry["tags"] = json!([operation.tag]);
        }
        if let Some(schema) = operation.query_schema {
            let schema = normalize_schema(schema, &mut components)?;
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                let parameters = properties.iter().map(|(name, property)| {
                    let required = schema.get("required").and_then(Value::as_array)
                        .is_some_and(|required| required.iter().any(|value| value.as_str() == Some(name)));
                    json!({"name": name, "in": "query", "required": required, "schema": property})
                }).collect::<Vec<_>>();
                entry["parameters"] = json!(parameters);
            }
        }
        if let Some(schema) = operation.body_schema {
            entry["requestBody"] = json!({"required": true, "content": {"application/json": {"schema": normalize_schema(schema, &mut components)?}}});
        }
        let path = paths.entry(operation.path).or_insert_with(|| json!({}));
        if path
            .as_object_mut()
            .unwrap()
            .insert(operation.method.to_lowercase(), entry)
            .is_some()
        {
            return Err(DocumentationError("duplicate documented route".into()));
        }
    }
    Ok(
        json!({"openapi": "3.1.0", "info": {"title": title, "version": version}, "paths": paths, "components": {"schemas": components}}),
    )
}
