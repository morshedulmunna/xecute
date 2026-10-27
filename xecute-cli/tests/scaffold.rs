//! End-to-end checks of generated project structure and formatting.

use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn new_project_is_formatted_locked_and_accepts_resources() {
    let root = std::env::temp_dir().join(format!(
        "xecute-scaffold-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_xecute");
    assert!(
        Command::new(binary)
            .args(["new", "sample-api"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    let project = root.join("sample-api");
    for file in [
        "Cargo.lock",
        "rust-toolchain.toml",
        "rustfmt.toml",
        "clippy.toml",
        ".editorconfig",
        "Dockerfile",
        ".github/workflows/ci.yml",
        "src/tests/server.rs",
        "src/tests/application.rs",
        "src/server/docs.rs",
        "src/middleware.rs",
        "src/configs/static_files.rs",
        "src/server/static_files.rs",
        "src/tests/static_files.rs",
        "public/hello.txt",
        "public/index.html",
        "src/configs/docs.rs",
        "src/configs/environment.rs",
        "src/modules/example/module.rs",
        "src/modules/example/controller.rs",
        "src/modules/example/tests/http.rs",
        ".env.example",
        "config/AppConfig.toml",
        "src/tests/versions.rs",
    ] {
        assert!(project.join(file).is_file(), "missing {file}");
    }
    for removed in [
        "src/modules/uploads",
        "src/modules/api_v2",
        "src/configs/uploads.rs",
        "src/tests/uploads.rs",
    ] {
        assert!(!project.join(removed).exists(), "unexpected {removed}");
    }
    assert!(!project.join("AppConfig.toml").exists());
    let manifest = fs::read_to_string(project.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("upload-s3"));
    assert!(!manifest.contains("features = [\"upload\"]"));
    assert!(!project.join("compose.yaml").exists());
    assert!(!project.join("docker-compose.yml").exists());
    let dockerfile = fs::read_to_string(project.join("Dockerfile")).unwrap();
    assert!(dockerfile.contains("gcr.io/distroless/static-debian13:nonroot"));
    assert!(dockerfile.contains("x86_64-unknown-linux-musl"));
    assert!(dockerfile.contains("aarch64-unknown-linux-musl"));
    assert!(dockerfile.contains("USER 65532:65532"));
    assert!(!dockerfile.contains("curl"));
    assert!(manifest.contains("version = \"0.1.8\""));
    assert!(!manifest.contains("path ="));
    assert!(
        Command::new(binary)
            .args(["g", "resource", "user"])
            .current_dir(&project)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("cargo")
            .args(["fmt", "--all", "--", "--check"])
            .current_dir(&project)
            .status()
            .unwrap()
            .success()
    );
    for file in [
        "src/modules/user/module.rs",
        "src/modules/user/dto/mod.rs",
        "src/modules/user/entity/mod.rs",
        "src/modules/user/repository/store.rs",
        "src/modules/user/tests/http.rs",
    ] {
        assert!(project.join(file).is_file(), "missing {file}");
    }
    assert!(!project.join("src/modules/user/user_module.rs").exists());
    let repository =
        fs::read_to_string(project.join("src/modules/user/repository/store.rs")).unwrap();
    assert!(repository.contains("#[derive(Injectable)]"));
    assert!(repository.contains("#[inject(default)]"));
    assert!(!repository.contains("impl Provider"));
    assert!(!repository.contains("impl Injectable"));
    let module = fs::read_to_string(project.join("src/modules/user/module.rs")).unwrap();
    assert!(module.contains("#[module(\n    imports = [\n        // Other feature modules, e.g. AuthModule (see import guide above).\n    ],\n    providers = [UserRepository, UserService, UserController],\n    controllers = [UserController],\n    exports = [UserService],\n)]"));
    let controller = fs::read_to_string(project.join("src/modules/user/controller.rs")).unwrap();
    assert!(controller.contains("#[controller(\"/user\")]"));
    assert!(controller.contains("#[derive(Injectable)]"));
    assert!(controller.contains("#[routes]"));
    assert!(controller.contains("#[body] request: CreateUserDto"));
    assert!(!controller.contains(".validate()"));
    assert!(!controller.contains("#[xecute::"));
    assert!(manifest.contains("missing_docs = \"warn\""));
    let main = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(main.contains("pub mod modules;"));
    assert!(
        fs::read_to_string(project.join("src/modules/mod.rs"))
            .unwrap()
            .contains("pub mod user;")
    );
    assert!(main.contains("server::run().await"));
    assert!(!main.contains("#[path"));
    let app = fs::read_to_string(project.join("src/app_module.rs")).unwrap();
    assert!(app.contains("UserModule::definition()"));
    assert!(app.contains("UserModule::scoped_router"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn architecture_selection_persists_and_invalid_flags_do_not_create_projects() {
    let root = std::env::temp_dir().join(format!("xecute-architecture-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_xecute");
    for (project, flags, architecture) in [
        ("cqrs-api", vec!["--cqrs"], "cqrs"),
        ("mvc-api", vec![], "mvc"),
        ("explicit-mvc-api", vec!["--mvc"], "mvc"),
    ] {
        assert!(
            Command::new(binary)
                .args(["new", project])
                .args(flags)
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        let project = root.join(project);
        assert_eq!(
            fs::read_to_string(project.join(".xecute/architecture"))
                .unwrap()
                .trim(),
            architecture
        );
        assert!(
            Command::new(binary)
                .args(["g", "res", "user-profile"])
                .current_dir(&project)
                .status()
                .unwrap()
                .success()
        );
        for feature in ["example", "user_profile"] {
            let feature = project.join("src/modules").join(feature);
            assert_eq!(feature.join("service.rs").exists(), architecture == "mvc");
            assert!(!feature.join("services").exists());
            if architecture == "cqrs" {
                for handler in ["commands/create.rs", "queries/list.rs"] {
                    let handler = fs::read_to_string(feature.join(handler)).unwrap();
                    assert!(handler.contains("repository: Arc<"));
                    assert!(!handler.contains("service: Arc<"));
                }
                let module = fs::read_to_string(feature.join("module.rs")).unwrap();
                assert!(!module.contains("use super::services::"));
                assert!(module.contains("exports = [Create"));
                assert!(module.contains("Repository,"));
            }
            assert_eq!(
                feature.join("commands/create.rs").exists(),
                architecture == "cqrs"
            );
            assert_eq!(
                feature.join("queries/list.rs").exists(),
                architecture == "cqrs"
            );
        }
        let before = fs::read_to_string(project.join("src/app_module.rs")).unwrap();
        fs::write(project.join(".xecute/architecture"), "invalid").unwrap();
        assert!(
            !Command::new(binary)
                .args(["g", "res", "invalid-config"])
                .current_dir(&project)
                .status()
                .unwrap()
                .success()
        );
        assert!(!project.join("src/invalid_config").exists());
        assert_eq!(
            fs::read_to_string(project.join("src/app_module.rs")).unwrap(),
            before
        );
    }
    for flags in [
        vec!["--bad"],
        vec!["--mvc", "--mvc"],
        vec!["--cqrs", "--cqrs"],
        vec!["--mvc", "--cqrs"],
        vec!["--cqrs", "--mvc"],
        vec!["--runtime-path"],
        vec!["--runtime-path", "--mvc"],
    ] {
        assert!(
            !Command::new(binary)
                .args(["new", "bad-api"])
                .args(flags)
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        assert!(!root.join("bad-api").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn initializes_current_directory_and_preflights_all_conflicts() {
    let root = std::env::temp_dir().join(format!(
        "xecute-in-place-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_xecute");
    for (name, flags, architecture) in [
        ("my-app", vec![], "mvc"),
        ("cqrs-app", vec!["--cqrs"], "cqrs"),
        ("Invalid Directory", vec!["--name", "valid-app"], "mvc"),
    ] {
        let project = root.join(name);
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::write(project.join("notes.txt"), "preserve me").unwrap();
        let output = Command::new(binary)
            .args(["new", "."])
            .args(flags)
            .current_dir(&project)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("  cd "));
        assert!(project.join("src/main.rs").is_file());
        assert!(!project.join("my-app").exists());
        assert_eq!(
            fs::read_to_string(project.join("notes.txt")).unwrap(),
            "preserve me"
        );
        assert_eq!(
            fs::read_to_string(project.join(".xecute/architecture"))
                .unwrap()
                .trim(),
            architecture
        );
        let package = if name == "Invalid Directory" {
            "valid-app"
        } else {
            name
        };
        assert!(
            fs::read_to_string(project.join("Cargo.toml"))
                .unwrap()
                .contains(&format!("name = \"{package}\""))
        );
    }
    for (index, conflict) in [
        "README.md",
        "Cargo.toml",
        "src/main.rs",
        "src/modules/example/tests/http.rs",
        ".github/workflows/ci.yml",
    ]
    .into_iter()
    .enumerate()
    {
        let project = root.join(format!("conflict-{index}"));
        fs::create_dir_all(project.join(conflict).parent().unwrap()).unwrap();
        fs::write(project.join(conflict), "preserve me").unwrap();
        let output = Command::new(binary)
            .args(["new", "."])
            .current_dir(&project)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("no files changed"));
        assert_eq!(
            fs::read_to_string(project.join(conflict)).unwrap(),
            "preserve me"
        );
        assert!(!project.join(".xecute").exists());
        if conflict != "Cargo.toml" {
            assert!(!project.join("Cargo.toml").exists());
        }
    }
    #[cfg(unix)]
    {
        let project = root.join("symlink-app");
        let outside = root.join("outside");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("src")).unwrap();
        assert!(
            !Command::new(binary)
                .args(["new", "."])
                .current_dir(&project)
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(!project.join("Cargo.toml").exists());
        assert!(fs::read_dir(outside).unwrap().next().is_none());
    }
    fs::remove_dir_all(root).unwrap();
}
