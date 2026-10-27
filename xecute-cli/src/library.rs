//! Generate reusable Cargo libraries without coupling them to an HTTP transport.
use std::{
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value, value};

type Result<T> = std::result::Result<T, String>;

pub fn generate(root: &Path, name: &str) -> Result<()> {
    let (crate_name, _) = crate::generate::names(name)?;
    let manifest_path = root.join("Cargo.toml");
    for path in [
        root.join(".xecute"),
        root.join(".xecute/workspace"),
        manifest_path.clone(),
    ] {
        let metadata = path.symlink_metadata().map_err(
            |_| "run xecute g library from a workspace created with xecute new --workspace",
        )?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{} must not be a symlink; no files changed",
                path.display()
            ));
        }
    }
    let lock_path = root.join(".xecute/library-generator.lock");
    let _lock_file = fs::OpenOptions::new().write(true).create_new(true).open(&lock_path)
        .map_err(|error| format!("cannot acquire library generator lock: {error}; another generation may be running; no files changed"))?;
    let _lock = GenerationLock(lock_path);
    let before = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let mut document = before
        .parse::<DocumentMut>()
        .map_err(|e| format!("invalid workspace manifest: {e}; no files changed"))?;
    let workspace = document
        .get_mut("workspace")
        .and_then(Item::as_table_mut)
        .ok_or("missing [workspace]; no files changed")?;
    let member = format!("libs/{name}");
    if let Some(exclude) = workspace.get("exclude") {
        let exclude = exclude
            .as_array()
            .ok_or("workspace.exclude must be an array; no files changed")?;
        if exclude.iter().any(|entry| {
            entry
                .as_str()
                .is_some_and(|pattern| matches_path(pattern, &member))
        }) {
            return Err("library path is excluded by workspace.exclude; no files changed".into());
        }
    }
    let members = workspace
        .get_mut("members")
        .and_then(Item::as_array_mut)
        .ok_or("workspace.members must be an array; no files changed")?;
    if !members.iter().any(|entry| {
        entry
            .as_str()
            .is_some_and(|pattern| matches_path(pattern, &member))
    }) {
        members.push(member.as_str());
    }
    if let Some(shared) = workspace.get("dependencies") {
        let shared = shared
            .as_table_like()
            .ok_or("invalid workspace.dependencies; no files changed")?;
        if shared
            .iter()
            .any(|(key, _)| key.replace('-', "_") == crate_name)
        {
            return Err(
                "workspace dependency already uses this library name; no files changed".into(),
            );
        }
    }
    let libs = root.join("libs");
    if let Ok(metadata) = libs.symlink_metadata()
        && (!metadata.is_dir() || metadata.file_type().is_symlink())
    {
        return Err("libs must be a real directory; no files changed".into());
    }
    let destination = libs.join(name);
    if destination.symlink_metadata().is_ok() {
        return Err("library destination already exists; no files changed".into());
    }
    // Ask Cargo about actual package names, including explicitly listed/custom members.
    let metadata = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot inspect workspace: {e}; no files changed"))?;
    if !metadata.status.success() {
        return Err(format!(
            "cannot inspect workspace; no files changed: {}",
            String::from_utf8_lossy(&metadata.stderr)
        ));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).map_err(|e| e.to_string())?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("invalid cargo metadata; no files changed")?;
    if packages.iter().any(|package| {
        package["name"]
            .as_str()
            .is_some_and(|existing| existing.replace('-', "_") == crate_name)
    }) {
        return Err("workspace package already uses this library name; no files changed".into());
    }
    let workspace = document["workspace"]
        .as_table_mut()
        .expect("validated workspace");
    if !workspace.contains_key("dependencies") {
        workspace.insert("dependencies", Item::Table(Table::new()));
    }
    let mut dependency = InlineTable::new();
    dependency.insert("path", Value::from(member.as_str()));
    workspace["dependencies"][name] = value(dependency);

    let manifest = library_manifest(name, &document)?;
    let source = include_str!("../templates/library/lib.rs").replace("__CRATE__", &crate_name);
    let readme = include_str!("../templates/library/readme.md")
        .replace("__NAME__", name)
        .replace("__CRATE__", &crate_name);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let stage = root.join(format!(".xecute/library-{}-{nonce}", std::process::id()));
    let temporary_manifest = stage.join("workspace.toml");
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        fs::create_dir(stage.join("src")).map_err(|e| e.to_string())?;
        fs::write(stage.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
        fs::write(stage.join("src/lib.rs"), source).map_err(|e| e.to_string())?;
        fs::write(stage.join("README.md"), readme).map_err(|e| e.to_string())?;
        crate::generate::format_files(&stage, &["src/lib.rs".into()])?;
        fs::write(&temporary_manifest, document.to_string()).map_err(|e| e.to_string())?;
        fs::set_permissions(
            &temporary_manifest,
            fs::metadata(&manifest_path)
                .map_err(|e| e.to_string())?
                .permissions(),
        )
        .map_err(|e| e.to_string())?;
        // Detect ordinary concurrent edits before committing the generated member.
        if fs::read_to_string(&manifest_path).map_err(|e| e.to_string())? != before {
            return Err(
                "workspace manifest changed during generation; retry; no files changed".into(),
            );
        }
        fs::create_dir_all(&libs).map_err(|e| e.to_string())?;
        // create_dir is exclusive: never replace a directory created by another generator.
        fs::create_dir(&destination)
            .map_err(|e| format!("cannot create library destination: {e}"))?;
        let commit = (|| {
            fs::rename(stage.join("src"), destination.join("src")).map_err(|e| e.to_string())?;
            fs::rename(stage.join("Cargo.toml"), destination.join("Cargo.toml"))
                .map_err(|e| e.to_string())?;
            fs::rename(stage.join("README.md"), destination.join("README.md"))
                .map_err(|e| e.to_string())?;
            fs::rename(&temporary_manifest, &manifest_path).map_err(|e| e.to_string())
        })();
        if commit.is_err() {
            let _ = fs::remove_dir_all(&destination);
        }
        commit
    })();
    let _ = fs::remove_dir_all(&stage);
    result?;
    let status = Command::new("cargo").args(["metadata", "--format-version", "1"])
        .stdout(std::process::Stdio::null()).current_dir(root).status()
        .map_err(|e| format!("library created, but lockfile refresh could not start: {e}; run cargo generate-lockfile"))?;
    if !status.success() {
        return Err("library created, but lockfile refresh failed; fix Cargo/registry configuration and run cargo generate-lockfile".into());
    }
    println!(
        "Created library {name} in libs/{name}\n  cargo test -p {name}\nAdd `{name} = {{ workspace = true }}` to an application's [dependencies].\nImport its Rust crate as `{crate_name}`."
    );
    Ok(())
}

fn library_manifest(name: &str, root: &DocumentMut) -> Result<String> {
    let mut document = DocumentMut::new();
    document["package"] = Item::Table(Table::new());
    document["package"]["name"] = value(name);
    document["package"]["version"] = value("0.1.0");
    document["package"]["publish"] = value(false);
    document["package"]["readme"] = value("README.md");
    for (key, fallback) in [("edition", "2024"), ("rust-version", "1.98.1")] {
        if root
            .get("workspace")
            .and_then(|w| w.get("package"))
            .and_then(|p| p.get(key))
            .is_some()
        {
            document["package"][key] = Item::Value(Value::InlineTable({
                let mut table = InlineTable::new();
                table.insert("workspace", true.into());
                table
            }));
        } else {
            document["package"][key] = value(fallback);
        }
    }
    if root.get("workspace").and_then(|w| w.get("lints")).is_some() {
        document["lints"]["workspace"] = value(true);
    } else {
        document["lints"]["rust"]["unsafe_code"] = value("forbid");
    }
    Ok(document.to_string())
}

fn matches_path(pattern: &str, path: &str) -> bool {
    glob::Pattern::new(pattern).is_ok_and(|pattern| {
        pattern.matches_with(
            path,
            glob::MatchOptions {
                case_sensitive: true,
                require_literal_separator: true,
                require_literal_leading_dot: true,
            },
        )
    })
}

// Exclusive scaffolding lock; crash leftovers are deliberately not stolen.
struct GenerationLock(std::path::PathBuf);
impl Drop for GenerationLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
