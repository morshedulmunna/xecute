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
        "src/server/docs.rs",
        "src/middleware.rs",
        "src/configs/docs.rs",
        "src/example/module.rs",
        "src/example/controller.rs",
        "src/tests/example.rs",
        ".env.example",
    ] {
        assert!(project.join(file).is_file(), "missing {file}");
    }
    let manifest = fs::read_to_string(project.join("Cargo.toml")).unwrap();
    assert!(!project.join("compose.yaml").exists());
    assert!(!project.join("docker-compose.yml").exists());
    let dockerfile = fs::read_to_string(project.join("Dockerfile")).unwrap();
    assert!(dockerfile.contains("gcr.io/distroless/static-debian13:nonroot"));
    assert!(dockerfile.contains("x86_64-unknown-linux-musl"));
    assert!(dockerfile.contains("aarch64-unknown-linux-musl"));
    assert!(dockerfile.contains("USER 65532:65532"));
    assert!(!dockerfile.contains("curl"));
    assert!(manifest.contains("xecute = \"0.1.4\""));
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
        "src/user/module.rs",
        "src/user/dto/mod.rs",
        "src/user/entity/mod.rs",
        "src/user/repository/store.rs",
        "src/tests/user.rs",
    ] {
        assert!(project.join(file).is_file(), "missing {file}");
    }
    assert!(!project.join("src/user/user_module.rs").exists());
    let repository = fs::read_to_string(project.join("src/user/repository/store.rs")).unwrap();
    assert!(repository.contains("#[derive(Injectable)]"));
    assert!(repository.contains("#[inject(default)]"));
    assert!(!repository.contains("impl Provider"));
    assert!(!repository.contains("impl Injectable"));
    let module = fs::read_to_string(project.join("src/user/module.rs")).unwrap();
    assert!(module.contains("#[module(\n    providers = [UserRepository, UserService, UserController],\n    controllers = [UserController],\n    exports = [UserService],\n)]"));
    let controller = fs::read_to_string(project.join("src/user/controller.rs")).unwrap();
    assert!(controller.contains("#[controller(\"/user\")]"));
    assert!(controller.contains("#[derive(Injectable)]"));
    assert!(controller.contains("#[routes]"));
    assert!(controller.contains("#[body] request: CreateUserDto"));
    assert!(!controller.contains(".validate()"));
    assert!(!controller.contains("#[xecute::"));
    assert!(manifest.contains("missing_docs = \"warn\""));
    let main = fs::read_to_string(project.join("src/main.rs")).unwrap();
    assert!(main.contains("pub mod user;"));
    assert!(main.contains("server::run().await"));
    assert!(!main.contains("#[path"));
    let app = fs::read_to_string(project.join("src/app_module.rs")).unwrap();
    assert!(app.contains("UserModule::definition()"));
    assert!(app.contains("UserController::router"));
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
            let feature = project.join("src").join(feature);
            assert_eq!(feature.join("service.rs").exists(), architecture == "mvc");
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
