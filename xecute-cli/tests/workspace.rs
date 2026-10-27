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
        let shared = fs::read_to_string(workspace.join("Cargo.toml")).unwrap();
        let shared: toml_edit::DocumentMut = shared.parse().unwrap();
        assert_eq!(
            shared["workspace"]["dependencies"]["axum"].as_str(),
            Some("0.8")
        );
        assert_eq!(
            shared["workspace"]["dependencies"]["xecute"]["path"].as_str(),
            runtime.to_str()
        );
        assert!(
            shared["workspace"]["dependencies"]["xecute"]
                .get("features")
                .is_none()
        );
        assert_eq!(
            shared["workspace"]["lints"]["rust"]["unsafe_code"].as_str(),
            Some("forbid")
        );
        assert_eq!(
            shared["workspace"]["lints"]["clippy"]["dbg_macro"].as_str(),
            Some("deny")
        );
        let before_shared = fs::read_to_string(workspace.join("Cargo.toml")).unwrap();
        let shared_files = [
            "rust-toolchain.toml",
            "rustfmt.toml",
            "clippy.toml",
            "deny.toml",
            ".editorconfig",
            ".gitattributes",
            ".dockerignore",
            ".gitignore",
            ".github/workflows/ci.yml",
            ".github/dependabot.yml",
        ];
        let shared_before: Vec<_> = shared_files
            .iter()
            .map(|file| fs::read(workspace.join(file)).unwrap())
            .collect();
        for file in shared_files {
            assert!(
                !workspace.join("apps").join(&project).join(file).exists(),
                "initial app duplicates {file}"
            );
        }

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
            let manifest: toml_edit::DocumentMut = fs::read_to_string(app.join("Cargo.toml"))
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(manifest["lints"]["workspace"].as_bool(), Some(true));
            assert_eq!(manifest["lints"].as_table().unwrap().len(), 1);
            for section in ["dependencies", "dev-dependencies"] {
                for (_, dependency) in manifest[section].as_table().unwrap() {
                    assert_eq!(dependency["workspace"].as_bool(), Some(true));
                    assert!(dependency.get("version").is_none());
                    assert!(dependency.get("path").is_none());
                }
            }
            assert!(
                manifest["dev-dependencies"]["xecute"]["features"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|feature| feature.as_str() == Some("testing"))
            );
            assert_eq!(
                fs::read_to_string(workspace.join("Cargo.toml")).unwrap(),
                before_shared
            );
            for (file, before) in shared_files.iter().zip(&shared_before) {
                assert!(!app.join(file).exists(), "new app duplicates {file}");
                assert_eq!(
                    &fs::read(workspace.join(file)).unwrap(),
                    before,
                    "shared file was rewritten: {file}"
                );
            }
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
    let output = Command::new(env!("CARGO_BIN_EXE_xecute"))
        .current_dir(&in_place)
        .args(["g", "lib", "shared-utils"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(in_place.join("libs/shared-utils/src/lib.rs").is_file());
    let output = Command::new("cargo")
        .current_dir(&in_place)
        .args(["test", "-p", "shared-utils", "--locked"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
