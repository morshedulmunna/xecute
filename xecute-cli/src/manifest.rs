//! Preserve Cargo manifest syntax while adding generated runtime and test dependencies.

use toml_edit::{Array, DocumentMut, InlineTable, Item, TableLike, Value, value};

/// Distributed worker transport selected when generating a worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkerBroker {
    /// Durable PostgreSQL outbox polled with `FOR UPDATE SKIP LOCKED`.
    PostgresOutbox,
    /// Durable RabbitMQ queue with retry republishing.
    RabbitMq,
    /// Core NATS queue group with at-most-once delivery.
    Nats,
}

impl WorkerBroker {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "postgres" | "postgres-outbox" => Some(Self::PostgresOutbox),
            "rabbitmq" | "rabbit-mq" | "amqp" => Some(Self::RabbitMq),
            "nats" => Some(Self::Nats),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::PostgresOutbox => "postgres-outbox",
            Self::RabbitMq => "rabbitmq",
            Self::Nats => "nats",
        }
    }
}

fn upgrade_runtime(item: &mut Item) {
    let old = |version: &str| {
        matches!(
            version,
            "0.1.0"
                | "0.1.1"
                | "0.1.2"
                | "0.1.3"
                | "0.1.4"
                | "0.1.5"
                | "0.1.6"
                | "0.1.7"
                | "0.1.8"
                | "0.1.9"
        )
    };
    if item.as_str().is_some_and(old) {
        *item = value("0.1.10");
    } else if let Some(table) = item.as_table_like_mut()
        && table.get("version").and_then(Item::as_str).is_some_and(old)
    {
        table.insert("version", value("0.1.10"));
    }
}
fn enable_feature(item: &mut Item, feature: &str) -> Result<(), String> {
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
        .any(|declared| declared.as_str() == Some(feature))
    {
        features.push(feature);
    }
    if let Some(inline) = item.as_inline_table_mut() {
        inline.fmt();
    }
    Ok(())
}

fn enable_testing(item: &mut Item) -> Result<(), String> {
    enable_feature(item, "testing")
}

pub(crate) fn testing_dependency(declaration: &str) -> Result<String, String> {
    let mut document = declaration
        .parse::<DocumentMut>()
        .map_err(|_| "invalid runtime declaration")?;
    enable_testing(&mut document["xecute"])?;
    Ok(document.to_string().trim().to_owned())
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

pub(crate) fn worker_dependencies(
    manifest: &str,
    broker: Option<WorkerBroker>,
) -> Result<String, String> {
    let mut document = manifest
        .parse::<DocumentMut>()
        .map_err(|_| "Cargo.toml is invalid; no files changed")?;
    let dependencies = document
        .get_mut("dependencies")
        .and_then(Item::as_table_like_mut)
        .ok_or("Cargo.toml is missing [dependencies]")?;
    let postgres = broker == Some(WorkerBroker::PostgresOutbox);
    let test_runtime = {
        let runtime = dependencies
            .get_mut("xecute")
            .ok_or("Cargo.toml is missing the xecute dependency")?;
        upgrade_runtime(runtime);
        enable_feature(runtime, "jobs")?;
        if postgres {
            enable_feature(runtime, "postgres")?;
        }
        let mut test_runtime = runtime.clone();
        enable_testing(&mut test_runtime)?;
        test_runtime
    };
    if let Some(broker) = broker {
        broker_dependencies(dependencies, broker);
    }
    if document.get("dev-dependencies").is_none() {
        document["dev-dependencies"] = Item::Table(toml_edit::Table::new());
    }
    let dev = document
        .get_mut("dev-dependencies")
        .and_then(Item::as_table_like_mut)
        .ok_or("invalid [dev-dependencies]")?;
    if let Some(existing) = dev.get_mut("xecute") {
        upgrade_runtime(existing);
        enable_feature(existing, "jobs")?;
        if postgres {
            enable_feature(existing, "postgres")?;
        }
        enable_testing(existing)?;
    } else {
        dev.insert("xecute", test_runtime);
    }
    Ok(document.to_string())
}

fn broker_dependencies(dependencies: &mut dyn TableLike, broker: WorkerBroker) {
    if !dependencies.contains_key("futures-util") && broker != WorkerBroker::PostgresOutbox {
        dependencies.insert("futures-util", value("0.3"));
    }
    match broker {
        WorkerBroker::PostgresOutbox => {}
        WorkerBroker::RabbitMq => {
            if !dependencies.contains_key("lapin") {
                let mut lapin = InlineTable::new();
                lapin.insert("version", "4.12".into());
                lapin.insert("default-features", false.into());
                let mut features = Array::new();
                for feature in ["tokio", "rustls-platform-verifier", "rustls--ring"] {
                    features.push(feature);
                }
                lapin.insert("features", features.into());
                lapin.fmt();
                dependencies.insert("lapin", Item::Value(Value::InlineTable(lapin)));
            }
        }
        WorkerBroker::Nats => {
            if !dependencies.contains_key("async-nats") {
                dependencies.insert("async-nats", value("0.50"));
            }
        }
    }
}

/// Lift dependency sources into the workspace, retaining member-specific features.
pub(crate) fn workspace_dependencies(
    manifest: &str,
    workspace: &str,
    runtime_override: bool,
) -> Result<(String, String), String> {
    let mut member = manifest.parse::<DocumentMut>().map_err(|e| e.to_string())?;
    let mut root = workspace
        .parse::<DocumentMut>()
        .map_err(|e| e.to_string())?;
    let workspace = root
        .get_mut("workspace")
        .and_then(Item::as_table_like_mut)
        .ok_or("missing [workspace]")?;
    if let Some(lints) = member.get("lints").cloned() {
        let inherited = lints.get("workspace").and_then(Item::as_bool) == Some(true);
        if !inherited {
            if !workspace.contains_key("lints") {
                workspace.insert("lints", lints);
            }
            let mut inheritance = toml_edit::Table::new();
            inheritance.insert("workspace", value(true));
            member["lints"] = Item::Table(inheritance);
        } else if !workspace.contains_key("lints") {
            return Err("workspace.lints is missing".into());
        }
    }
    if !workspace.contains_key("dependencies") {
        workspace.insert("dependencies", Item::Table(toml_edit::Table::new()));
    }
    let shared = workspace
        .get_mut("dependencies")
        .and_then(Item::as_table_like_mut)
        .ok_or("invalid [workspace.dependencies]")?;
    for section in ["dependencies", "dev-dependencies"] {
        let Some(dependencies) = member.get_mut(section).and_then(Item::as_table_like_mut) else {
            continue;
        };
        for (key, dependency) in dependencies.iter_mut() {
            let name = key.get();
            let inherited = dependency
                .as_table_like()
                .and_then(|t| t.get("workspace"))
                .and_then(Item::as_bool)
                == Some(true);
            if !inherited
                && (!shared.contains_key(name)
                    || (runtime_override && name == "xecute" && section == "dependencies"))
            {
                let mut source = dependency.clone();
                if let Some(table) = source.as_table_like_mut() {
                    // These apply at the app level, and optional is forbidden at workspace level.
                    table.remove("features");
                    table.remove("optional");
                }
                shared.insert(name, source);
            }
            if !shared.contains_key(name) {
                return Err(format!("workspace.dependencies is missing {name}"));
            }
            let mut reference = InlineTable::new();
            reference.insert("workspace", true.into());
            if let Some(table) = dependency.as_table_like() {
                for key in ["features", "optional"] {
                    if let Some(value) = table.get(key).and_then(Item::as_value) {
                        reference.insert(key, value.clone());
                    }
                }
                // Cargo cannot disable defaults in a member if the workspace enables them.
                if table.get("default-features").and_then(Item::as_bool) == Some(false)
                    && shared
                        .get(name)
                        .and_then(Item::as_table_like)
                        .and_then(|t| t.get("default-features"))
                        .and_then(Item::as_bool)
                        != Some(false)
                {
                    return Err(format!(
                        "set default-features = false for {name} in workspace.dependencies first"
                    ));
                }
            }
            reference.fmt();
            *dependency = Item::Value(Value::InlineTable(reference));
        }
    }
    Ok((member.to_string(), root.to_string()))
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
    fn worker_adds_jobs_features_and_preserves_existing_entries() {
        for declaration in [
            "xecute = \"0.1.6\"",
            "xecute = { path = \"../runtime\", features = [\"cache\"] }",
            "[dependencies.xecute]\npath = \"../runtime\"",
        ] {
            let input = format!(
                "# keep this\n[package]\nname = \"example\"\n[dependencies]\n{declaration}\n"
            );
            let output = worker_dependencies(&input, None).unwrap();
            let doc = output.parse::<DocumentMut>().unwrap();
            assert!(output.contains("# keep this"));
            assert!(
                doc["dependencies"]["xecute"]["features"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some("jobs"))
            );
            for feature in ["jobs", "testing"] {
                assert!(
                    doc["dev-dependencies"]["xecute"]["features"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value.as_str() == Some(feature)),
                    "missing dev feature {feature}"
                );
            }
            if declaration.contains("cache") {
                assert!(
                    doc["dependencies"]["xecute"]["features"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value.as_str() == Some("cache"))
                );
            }
            assert_eq!(worker_dependencies(&output, None).unwrap(), output);
        }
    }

    #[test]
    fn distributed_workers_add_the_selected_broker_dependencies() {
        let input = "[package]\nname = \"example\"\n[dependencies]\nxecute = \"0.1.10\"\n";
        let postgres = worker_dependencies(input, Some(WorkerBroker::PostgresOutbox)).unwrap();
        let doc = postgres.parse::<DocumentMut>().unwrap();
        for feature in ["jobs", "postgres"] {
            assert!(
                doc["dependencies"]["xecute"]["features"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some(feature))
            );
        }
        assert!(
            !doc["dependencies"]
                .as_table()
                .unwrap()
                .contains_key("lapin")
        );

        for (broker, dependency) in [
            (WorkerBroker::RabbitMq, "lapin"),
            (WorkerBroker::Nats, "async-nats"),
        ] {
            let output = worker_dependencies(input, Some(broker)).unwrap();
            let doc = output.parse::<DocumentMut>().unwrap();
            assert!(
                doc["dependencies"]
                    .as_table()
                    .unwrap()
                    .contains_key(dependency)
            );
            assert_eq!(doc["dependencies"]["futures-util"].as_str(), Some("0.3"));
            assert!(
                doc["dev-dependencies"]["xecute"]["features"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some("testing"))
            );
        }
        let rabbit = worker_dependencies(input, Some(WorkerBroker::RabbitMq)).unwrap();
        let doc = rabbit.parse::<DocumentMut>().unwrap();
        assert_eq!(
            doc["dependencies"]["lapin"]["version"].as_str(),
            Some("4.12")
        );
        assert_eq!(
            doc["dependencies"]["lapin"]["default-features"].as_bool(),
            Some(false)
        );
        assert!(
            doc["dependencies"]["lapin"]["features"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_str() == Some("tokio"))
        );
        let nats = worker_dependencies(input, Some(WorkerBroker::Nats)).unwrap();
        let doc = nats.parse::<DocumentMut>().unwrap();
        assert_eq!(doc["dependencies"]["async-nats"].as_str(), Some("0.50"));
    }

    #[test]
    fn workspace_sources_are_shared_and_member_features_are_preserved() {
        let member = r#"
[dependencies]
xecute = { path = "/runtime", features = ["custom"] }
serde = { version = "1", features = ["derive"] }
[dev-dependencies]
xecute = { path = "/runtime", features = ["testing"] }
"#;
        let root =
            "[workspace]\nmembers = [\"apps/*\"]\n[workspace.dependencies]\nserde = \"=1.0.228\"\n";
        let (member, root) = workspace_dependencies(member, root, false).unwrap();
        let doc = root.parse::<DocumentMut>().unwrap();
        assert_eq!(
            doc["workspace"]["dependencies"]["serde"].as_str(),
            Some("=1.0.228")
        );
        assert!(
            doc["workspace"]["dependencies"]["xecute"]
                .get("features")
                .is_none()
        );
        let doc = member.parse::<DocumentMut>().unwrap();
        assert_eq!(
            doc["dependencies"]["serde"]["workspace"].as_bool(),
            Some(true)
        );
        assert_eq!(
            doc["dependencies"]["serde"]["features"][0].as_str(),
            Some("derive")
        );
        assert_eq!(
            doc["dev-dependencies"]["xecute"]["features"][0].as_str(),
            Some("testing")
        );
        assert_eq!(
            workspace_dependencies(&member, &root, false).unwrap(),
            (member, root.clone())
        );
        let (_, overridden) = workspace_dependencies(
            "[dependencies]\nxecute = { path = \"/replacement\" }\n",
            &root,
            true,
        )
        .unwrap();
        let doc = overridden.parse::<DocumentMut>().unwrap();
        assert_eq!(
            doc["workspace"]["dependencies"]["xecute"]["path"].as_str(),
            Some("/replacement")
        );
    }

    #[test]
    fn workspace_lints_are_inherited_without_overriding_root_policy() {
        let input = "[lints.rust]\nunsafe_code = \"forbid\"\nmissing_docs = \"warn\"\n[lints.clippy]\ndbg_macro = \"deny\"\ntodo = \"deny\"\nunimplemented = \"deny\"\n";
        let (member, root) =
            workspace_dependencies(input, "[workspace]\nmembers = []\n", false).unwrap();
        let doc = member.parse::<DocumentMut>().unwrap();
        assert_eq!(doc["lints"]["workspace"].as_bool(), Some(true));
        assert_eq!(doc["lints"].as_table().unwrap().len(), 1);
        let doc = root.parse::<DocumentMut>().unwrap();
        assert_eq!(
            doc["workspace"]["lints"]["rust"]["unsafe_code"].as_str(),
            Some("forbid")
        );
        assert_eq!(
            doc["workspace"]["lints"]["clippy"]["todo"].as_str(),
            Some("deny")
        );
        assert_eq!(
            workspace_dependencies(&member, &root, false).unwrap(),
            (member, root.clone())
        );
        let customized = root.replace("missing_docs = \"warn\"", "missing_docs = \"deny\"");
        let (_, after) = workspace_dependencies(input, &customized, false).unwrap();
        assert_eq!(after, customized);
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
