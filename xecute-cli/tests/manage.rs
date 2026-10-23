//! Verify self-management targets the running installation without touching projects.
#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn self_management_uses_registered_installation_and_propagates_cargo_failures() {
    let root = std::env::temp_dir().join(format!(
        "xecute-manage-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let installation = root.join("custom cargo root");
    let fake = root.join("fake");
    let project = root.join("project");
    for folder in [installation.join("bin"), fake.clone(), project.clone()] {
        fs::create_dir_all(folder).unwrap();
    }
    let executable = installation.join("bin/xecute");
    fs::copy(env!("CARGO_BIN_EXE_xecute"), &executable).unwrap();
    fs::write(installation.join(".crates2.json"), r#"{"installs":{"xecute-cli 0.1.7 (registry+https://github.com/rust-lang/crates.io-index)":{"bins":["xecute"]}}}"#).unwrap();
    fs::write(project.join("keep.txt"), "preserved").unwrap();
    let cargo = fake.join("cargo");
    fs::write(&cargo, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$XECUTE_TEST_LOG\"\nexit \"${XECUTE_TEST_STATUS:-0}\"\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let log = root.join("args.txt");
    let path = format!(
        "{}:{}",
        fake.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    for (action, expected) in [
        (
            "update",
            vec![
                "install",
                "xecute-cli",
                "--registry",
                "crates-io",
                "--locked",
                "--force",
            ],
        ),
        ("uninstall", vec!["uninstall", "xecute-cli"]),
    ] {
        let output = Command::new(&executable)
            .arg(action)
            .current_dir(&project)
            .env("PATH", &path)
            .env("XECUTE_TEST_LOG", &log)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let arguments = fs::read_to_string(&log).unwrap();
        let mut expected: Vec<String> = expected.into_iter().map(String::from).collect();
        expected.extend([
            "--root".into(),
            installation
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ]);
        assert_eq!(arguments.lines().collect::<Vec<_>>(), expected);
        assert_eq!(
            fs::read_to_string(project.join("keep.txt")).unwrap(),
            "preserved"
        );
        let output = Command::new(&executable)
            .arg(action)
            .current_dir(&project)
            .env("PATH", &path)
            .env("XECUTE_TEST_LOG", &log)
            .env("XECUTE_TEST_STATUS", "7")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Cargo failed"));
    }
    // Refuse unregistered executables and extra arguments before invoking Cargo.
    fs::remove_file(&log).unwrap();
    let output = Command::new(&executable)
        .args(["uninstall", "--all"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!log.exists());
    fs::remove_file(installation.join(".crates2.json")).unwrap();
    let output = Command::new(&executable)
        .arg("uninstall")
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!log.exists());
    fs::remove_dir_all(root).unwrap();
}
