//! Setup must resolve a fresh graph, and must not report success on resolver failure.
#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn setup_resolves_fresh_dependencies_and_reports_failure() {
    let root = std::env::temp_dir().join(format!(
        "xecute-resolution-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let shim = bin.join("cargo");
    fs::write(
        &shim,
        r#"#!/bin/sh
[ "$#" -eq 1 ] && [ "$1" = generate-lockfile ] || exit 91
# A bundled snapshot must never be present before fresh resolution.
[ ! -e Cargo.lock ] || exit 92
[ -f src/modules/example/controller.rs ] || exit 93
if [ "$XECUTE_TEST_RESOLUTION_FAIL" = 1 ]; then exit 42; fi
printf '%s\n' '# freshly resolved by test registry' > Cargo.lock
"#,
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    for (name, architecture, local, fail) in [
        ("mvc-app", "--mvc", false, false),
        ("cqrs-app", "--cqrs", false, false),
        ("local-app", "--mvc", true, false),
        ("failed-app", "--cqrs", false, true),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xecute"));
        command
            .args(["new", name, architecture])
            .current_dir(&root)
            .env("PATH", &path)
            .env("XECUTE_TEST_RESOLUTION_FAIL", if fail { "1" } else { "0" });
        if local {
            command
                .arg("--runtime-path")
                .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../xecute"));
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), !fail, "{name}: {stderr}");
        let project = root.join(name);
        assert!(project.join("Cargo.toml").is_file());
        if fail {
            assert!(!project.join("Cargo.lock").exists());
            assert!(stderr.contains("project files created, but dependency resolution failed"));
            assert!(stderr.contains("cargo generate-lockfile"));
            assert!(!String::from_utf8_lossy(&output.stdout).contains("Created failed-app"));
        } else {
            assert_eq!(
                fs::read_to_string(project.join("Cargo.lock")).unwrap(),
                "# freshly resolved by test registry\n"
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}
