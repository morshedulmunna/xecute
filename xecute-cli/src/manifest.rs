//! Preserve Cargo manifest syntax while adding generated runtime and test dependencies.

use toml_edit::{Array, DocumentMut, InlineTable, Item, Value, value};

fn upgrade_runtime(item: &mut Item) {
    let old = |version: &str| {
        matches!(
            version,
            "0.1.0" | "0.1.1" | "0.1.2" | "0.1.3" | "0.1.4" | "0.1.5" | "0.1.6"
        )
    };
    if item.as_str().is_some_and(old) {
        *item = value("0.1.7");
    } else if let Some(table) = item.as_table_like_mut()
        && table.get("version").and_then(Item::as_str).is_some_and(old)
    {
        table.insert("version", value("0.1.7"));
    }
}
fn enable_testing(item: &mut Item) -> Result<(), String> {
    if let Some(version) = item.as_str() {
        let mut table = InlineTable::new();
        table.insert("version", Value::from(version));
        *item = Item::Value(Value::InlineTable(table));
    }
    let table = item
        .as_table_like_mut()
        .ok_or("xecute dependency must be a version or table")?;
    if !table.contains_key("features") {
        table.insert("features", value(Array::new()));
    }
    let features = table
        .get_mut("features")
        .and_then(Item::as_array_mut)
        .ok_or("xecute features must be an array")?;
    if !features
        .iter()
        .any(|feature| feature.as_str() == Some("testing"))
    {
        features.push("testing");
    }
    if let Some(inline) = item.as_inline_table_mut() {
        inline.fmt();
    }
    Ok(())
}

pub(crate) fn resource_dependencies(manifest: &str) -> Result<String, String> {
    let mut document = manifest
        .parse::<DocumentMut>()
        .map_err(|_| "Cargo.toml is invalid; no files changed")?;
    let dependencies = document
        .get_mut("dependencies")
        .and_then(Item::as_table_like_mut)
        .ok_or("Cargo.toml is missing [dependencies]")?;
    if !dependencies.contains_key("validator") {
        let mut validator = InlineTable::new();
        validator.insert("version", "0.20".into());
        let mut features = Array::new();
        features.push("derive");
        validator.insert("features", features.into());
        validator.fmt();
        dependencies.insert("validator", Item::Value(Value::InlineTable(validator)));
    }
    if !dependencies.contains_key("schemars") {
        dependencies.insert("schemars", value("1.2"));
    }
    let runtime = dependencies
        .get_mut("xecute")
        .ok_or("Cargo.toml is missing the xecute dependency")?;
    upgrade_runtime(runtime);
    let mut test_runtime = runtime.clone();
    enable_testing(&mut test_runtime)?;
    if document.get("dev-dependencies").is_none() {
        document["dev-dependencies"] = Item::Table(toml_edit::Table::new());
    }
    let dev = document
        .get_mut("dev-dependencies")
        .and_then(Item::as_table_like_mut)
        .ok_or("invalid [dev-dependencies]")?;
    if let Some(existing) = dev.get_mut("xecute") {
        upgrade_runtime(existing);
        enable_testing(existing)?;
    } else {
        dev.insert("xecute", test_runtime);
    }
    Ok(document.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adds_test_feature_preserves_custom_paths_features_and_comments() {
        for declaration in [
            "xecute = \"0.1.6\"",
            "xecute = { path = \"../runtime\", features = [\"custom\"] }",
            "[dependencies.xecute]\npath = \"../runtime\"",
        ] {
            let input = format!(
                "# keep this\n[package]\nname = \"example\"\n[dependencies]\n{declaration}\n"
            );
            let output = resource_dependencies(&input).unwrap();
            let doc = output.parse::<DocumentMut>().unwrap();
            assert!(output.contains("# keep this"));
            assert!(
                doc["dev-dependencies"]["xecute"]["features"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some("testing"))
            );
            assert_eq!(resource_dependencies(&output).unwrap(), output);
            if declaration.contains("path") {
                assert_eq!(
                    doc["dev-dependencies"]["xecute"]["path"].as_str(),
                    Some("../runtime")
                );
            }
        }
    }
    #[test]
    fn invalid_manifests_and_features_are_rejected() {
        assert!(resource_dependencies("not toml").is_err());
        assert!(
            resource_dependencies(
                "[dependencies]\nxecute = { version = \"0.1.6\", features = false }"
            )
            .is_err()
        );
    }
}
