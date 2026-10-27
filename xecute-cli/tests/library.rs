//! A library is an actual workspace crate that applications can consume.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new(extra: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "xecute-library-{}-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join(".xecute")).unwrap();
        fs::write(root.join(".xecute/workspace"), "1\n").unwrap();
        fs::create_dir_all(root.join("apps/api/src")).unwrap();
        fs::write(
            root.join("apps/api/Cargo.toml"),
            "[package]\nname = \"api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(root.join("apps/api/src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("Cargo.toml"), format!("# keep my comment\n[workspace]\nmembers = [\"apps/*\"]\nresolver = \"3\"\n{extra}\n")).unwrap();
        Self(root)
    }
    fn generate(&self, alias: &str, name: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_xecute"))
            .args(["g", alias, name])
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cargo(root: &Path, args: &[&str]) {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn library_is_registered_consumable_private_formatted_and_tested() {
    let workspace = Workspace::new(
        "[workspace.package]\nedition = \"2024\"\nrust-version = \"1.98.1\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n[workspace.dependencies]\n# keep existing declaration\n",
    );
    let app_before = fs::read(workspace.0.join("apps/api/Cargo.toml")).unwrap();
    let output = workspace.generate("library", "shared-utils");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(workspace.0.join("apps/api/Cargo.toml")).unwrap(),
        app_before
    );
    let root = fs::read_to_string(workspace.0.join("Cargo.toml")).unwrap();
    assert!(root.contains("# keep my comment") && root.contains("# keep existing declaration"));
    let document: toml_edit::DocumentMut = root.parse().unwrap();
    assert_eq!(
        document["workspace"]["dependencies"]["shared-utils"]["path"].as_str(),
        Some("libs/shared-utils")
    );
    let lib: toml_edit::DocumentMut =
        fs::read_to_string(workspace.0.join("libs/shared-utils/Cargo.toml"))
            .unwrap()
            .parse()
            .unwrap();
    assert_eq!(lib["package"]["publish"].as_bool(), Some(false));
    assert_eq!(lib["package"]["edition"]["workspace"].as_bool(), Some(true));
    assert_eq!(lib["lints"]["workspace"].as_bool(), Some(true));
    assert!(!workspace.0.join("libs/shared-utils/Cargo.lock").exists());
    fs::write(workspace.0.join("apps/api/Cargo.toml"), "[package]\nname = \"api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nshared-utils = { workspace = true }\n").unwrap();
    fs::write(
        workspace.0.join("apps/api/src/main.rs"),
        "fn main() {\n    assert_eq!(shared_utils::greet(\"  Munna  \"), \"Hello, Munna!\");\n}\n",
    )
    .unwrap();
    cargo(&workspace.0, &["generate-lockfile", "--offline"]);
    cargo(&workspace.0, &["run", "-p", "api", "--locked", "--offline"]);
    cargo(
        &workspace.0,
        &["test", "-p", "shared-utils", "--locked", "--offline"],
    );
    cargo(
        &workspace.0,
        &[
            "clippy",
            "-p",
            "shared-utils",
            "--all-targets",
            "--locked",
            "--offline",
            "--",
            "-D",
            "warnings",
        ],
    );
    cargo(&workspace.0, &["fmt", "--all", "--", "--check"]);
    let before = fs::read(workspace.0.join("Cargo.toml")).unwrap();
    assert!(!workspace.generate("lib", "shared-utils").status.success());
    assert_eq!(before, fs::read(workspace.0.join("Cargo.toml")).unwrap());
}

#[test]
fn aliases_fallback_settings_and_normalized_name_conflicts() {
    let workspace = Workspace::new("");
    assert!(workspace.generate("lib", "shared_data").status.success());
    let lib: toml_edit::DocumentMut =
        fs::read_to_string(workspace.0.join("libs/shared_data/Cargo.toml"))
            .unwrap()
            .parse()
            .unwrap();
    assert_eq!(lib["package"]["edition"].as_str(), Some("2024"));
    assert_eq!(lib["lints"]["rust"]["unsafe_code"].as_str(), Some("forbid"));
    for name in [
        "shared-data",
        "api",
        "../outside",
        "type",
        "BadName",
        "bad__name",
    ] {
        let before = fs::read(workspace.0.join("Cargo.toml")).unwrap();
        assert!(!workspace.generate("lib", name).status.success(), "{name}");
        assert_eq!(before, fs::read(workspace.0.join("Cargo.toml")).unwrap());
    }
    let metadata = Command::new("cargo")
        .args(["metadata", "--no-deps", "--locked", "--format-version", "1"])
        .current_dir(&workspace.0)
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let data: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    assert_eq!(data["packages"].as_array().unwrap().len(), 2);
}

#[test]
fn excludes_conflicts_and_invalid_context_are_rejected_without_mutation() {
    for extra in [
        "exclude = [\"libs/private-*\"]",
        "[workspace.dependencies]\nprivate-utils = \"1\"",
    ] {
        let workspace = Workspace::new(extra);
        let before = fs::read(workspace.0.join("Cargo.toml")).unwrap();
        assert!(
            !workspace
                .generate("library", "private-utils")
                .status
                .success()
        );
        assert_eq!(before, fs::read(workspace.0.join("Cargo.toml")).unwrap());
        assert!(!workspace.0.join("libs/private-utils").exists());
    }
    let workspace = Workspace::new("");
    fs::remove_file(workspace.0.join(".xecute/workspace")).unwrap();
    assert!(!workspace.generate("lib", "shared").status.success());
    assert!(!workspace.0.join("libs").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_destinations_and_manifests_are_never_followed() {
    use std::os::unix::fs::symlink;
    let workspace = Workspace::new("");
    let external = Workspace::new("");
    let before = fs::read(workspace.0.join("Cargo.toml")).unwrap();
    symlink(&external.0, workspace.0.join("libs")).unwrap();
    assert!(!workspace.generate("lib", "shared").status.success());
    assert!(!external.0.join("shared").exists());
    assert_eq!(before, fs::read(workspace.0.join("Cargo.toml")).unwrap());
    fs::remove_file(workspace.0.join("libs")).unwrap();
    fs::create_dir(workspace.0.join("libs")).unwrap();
    symlink(workspace.0.join("missing"), workspace.0.join("libs/shared")).unwrap();
    assert!(!workspace.generate("lib", "shared").status.success());
    fs::remove_file(workspace.0.join("Cargo.toml")).unwrap();
    symlink(
        external.0.join("Cargo.toml"),
        workspace.0.join("Cargo.toml"),
    )
    .unwrap();
    assert!(!workspace.generate("lib", "other").status.success());
}

#[test]
fn concurrent_generation_lock_and_extra_arguments_do_not_change_workspace() {
    let workspace = Workspace::new("");
    fs::write(
        workspace.0.join(".xecute/library-generator.lock"),
        "another generator",
    )
    .unwrap();
    let before = fs::read(workspace.0.join("Cargo.toml")).unwrap();
    assert!(!workspace.generate("lib", "shared").status.success());
    assert_eq!(before, fs::read(workspace.0.join("Cargo.toml")).unwrap());
    assert!(!workspace.0.join("libs").exists());
    assert_eq!(
        fs::read_to_string(workspace.0.join(".xecute/library-generator.lock")).unwrap(),
        "another generator"
    );
    for args in [
        vec!["g", "lib"],
        vec!["g", "library", "shared", "--unknown"],
    ] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_xecute"))
                .args(args)
                .current_dir(&workspace.0)
                .status()
                .unwrap()
                .success()
        );
    }
}

#[cfg(unix)]
#[test]
fn formatting_failure_rolls_back_and_lockfile_failure_preserves_recoverable_output() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = Workspace::new("");
    let bin = workspace.0.join("bin");
    fs::create_dir(&bin).unwrap();
    let formatter = bin.join("rustfmt");
    fs::write(&formatter, "#!/bin/sh\nexit 19\n").unwrap();
    fs::set_permissions(&formatter, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin.clone())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let before = fs::read(workspace.0.join("Cargo.toml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xecute"))
        .args(["g", "lib", "shared"])
        .current_dir(&workspace.0)
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(before, fs::read(workspace.0.join("Cargo.toml")).unwrap());
    assert!(!workspace.0.join("libs/shared").exists());
    assert_eq!(
        fs::read_dir(workspace.0.join(".xecute")).unwrap().count(),
        1
    );
    fs::remove_file(formatter).unwrap();
    let cargo = bin.join("cargo");
    fs::write(&cargo, "#!/bin/sh\ncase \"$*\" in *--no-deps*) exec \"$XECUTE_REAL_CARGO\" \"$@\" ;; *) exit 19 ;; esac\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xecute"))
        .args(["generate", "library", "shared"])
        .current_dir(&workspace.0)
        .env("PATH", path)
        .env("XECUTE_REAL_CARGO", env!("CARGO"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("library created, but lockfile refresh failed")
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Created library"));
    assert!(workspace.0.join("libs/shared/src/lib.rs").is_file());
    assert!(!workspace.0.join(".xecute/library-generator.lock").exists());
    cargo_check_recovery(&workspace.0);
}

#[cfg(unix)]
fn cargo_check_recovery(root: &Path) {
    cargo(root, &["generate-lockfile", "--offline"]);
    cargo(root, &["test", "-p", "shared", "--locked", "--offline"]);
}

#[test]
fn existing_library_member_globs_are_preserved_without_duplicate_entries() {
    let workspace = Workspace::new("");
    let first = workspace.generate("lib", "first");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let path = workspace.0.join("Cargo.toml");
    let mut document: toml_edit::DocumentMut = fs::read_to_string(&path).unwrap().parse().unwrap();
    let mut members = toml_edit::Array::new();
    members.push("apps/*");
    members.push("libs/*");
    document["workspace"]["members"] = toml_edit::value(members);
    fs::write(&path, document.to_string()).unwrap();
    let output = workspace.generate("lib", "second");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: toml_edit::DocumentMut = fs::read_to_string(&path).unwrap().parse().unwrap();
    assert_eq!(
        document["workspace"]["members"].as_array().unwrap().len(),
        2
    );
    assert_eq!(document["workspace"]["members"][1].as_str(), Some("libs/*"));
    cargo(
        &workspace.0,
        &["test", "--workspace", "--locked", "--offline"],
    );
}
