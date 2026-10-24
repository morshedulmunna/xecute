//! Workspace creation, application architecture, and Cargo membership checks.
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn workspace_apps_support_both_architectures() {
    let root = std::env::temp_dir().join(format!(
        "xecute-workspace-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("xecute");
    for style in ["mvc", "cqrs"] {
        let project = format!("sample-{style}");
        let mut create = Command::new(env!("CARGO_BIN_EXE_xecute"));
        create
            .current_dir(&root)
            .args(["new", &project, "--workspace"]);
        // Exercise MVC as the default, without an architecture flag.
        if style == "cqrs" {
            create.arg("--cqrs");
        }
        let output = create.arg("--runtime-path").arg(&runtime).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let workspace = root.join(&project);
        for (app, flag, expected) in [
            ("inherited", None, style),
            (
                "override-app",
                Some(if style == "mvc" { "--cqrs" } else { "--mvc" }),
                if style == "mvc" { "cqrs" } else { "mvc" },
            ),
        ] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_xecute"));
            command.current_dir(&workspace).args(["g", "apps", app]);
            if let Some(flag) = flag {
                command.arg(flag);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let app = workspace.join("apps").join(app);
            assert_eq!(
                fs::read_to_string(app.join(".xecute/architecture"))
                    .unwrap()
                    .trim(),
                expected
            );
            assert_eq!(
                app.join("src/modules/example/commands/create.rs").exists(),
                expected == "cqrs"
            );
            assert_eq!(
                app.join("src/modules/example/service.rs").exists(),
                expected == "mvc"
            );
            let output = Command::new(env!("CARGO_BIN_EXE_xecute"))
                .current_dir(&app)
                .args(["g", "resource", "user"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                app.join("src/modules/user/commands/create.rs").exists(),
                expected == "cqrs"
            );
            assert!(!app.join("Cargo.lock").exists());
            assert!(
                !fs::read_to_string(app.join("Cargo.toml"))
                    .unwrap()
                    .contains("[workspace]")
            );
        }
        let metadata = Command::new("cargo")
            .current_dir(&workspace)
            .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
            .output()
            .unwrap();
        assert!(
            metadata.status.success(),
            "{}",
            String::from_utf8_lossy(&metadata.stderr)
        );
        let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
        assert_eq!(metadata["workspace_members"].as_array().unwrap().len(), 3);
        let before = fs::read(workspace.join("Cargo.lock")).unwrap();
        for args in [
            vec!["g", "apps", "inherited"],
            vec!["g", "apps", "../escape"],
            vec!["g", "apps", "bad", "--mvc", "--cqrs"],
        ] {
            assert!(
                !Command::new(env!("CARGO_BIN_EXE_xecute"))
                    .current_dir(&workspace)
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        }
        assert_eq!(before, fs::read(workspace.join("Cargo.lock")).unwrap());
        let fmt = Command::new("cargo")
            .current_dir(&workspace)
            .args(["fmt", "--all", "--", "--check"])
            .output()
            .unwrap();
        assert!(
            fmt.status.success(),
            "{}",
            String::from_utf8_lossy(&fmt.stdout)
        );
    }
    let in_place = root.join("existing-directory");
    fs::create_dir(&in_place).unwrap();
    fs::write(in_place.join("keep.txt"), "preserved").unwrap();
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_xecute"))
            .current_dir(&in_place)
            .args(["g", "apps", "outside"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_xecute"))
        .current_dir(&in_place)
        .args([
            "new",
            ".",
            "--workspace",
            "--name",
            "first-app",
            "--cqrs",
            "--runtime-path",
        ])
        .arg(&runtime)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(in_place.join("apps/first-app/src/main.rs").is_file());
    assert_eq!(
        fs::read_to_string(in_place.join("keep.txt")).unwrap(),
        "preserved"
    );
    assert!(
        !Command::new(env!("CARGO_BIN_EXE_xecute"))
            .current_dir(&in_place)
            .args(["new", ".", "--workspace"])
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
