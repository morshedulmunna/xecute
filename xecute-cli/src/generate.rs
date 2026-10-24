use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, String>;

// Repository-wide settings are emitted once, at the workspace root.
const WORKSPACE_FILES: &[(&str, &str)] = &[
    (
        "rust-toolchain.toml",
        include_str!("../templates/rust-toolchain.toml"),
    ),
    ("rustfmt.toml", include_str!("../templates/rustfmt.toml")),
    ("clippy.toml", include_str!("../templates/clippy.toml")),
    ("deny.toml", include_str!("../templates/deny.toml")),
    (".editorconfig", include_str!("../templates/editorconfig")),
    (".gitattributes", include_str!("../templates/gitattributes")),
    (".dockerignore", include_str!("../templates/dockerignore")),
    (
        ".github/dependabot.yml",
        include_str!("../templates/dependabot.yml"),
    ),
];

/// Project generation style, persisted for subsequent resource generators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture {
    Cqrs,
    Mvc,
}

impl Architecture {
    fn name(self) -> &'static str {
        match self {
            Self::Cqrs => "cqrs",
            Self::Mvc => "mvc",
        }
    }

    pub(crate) fn read(root: &Path) -> Result<Self> {
        match fs::read_to_string(root.join(".xecute/architecture")) {
            Ok(value) => match value.trim() {
                "cqrs" => Ok(Self::Cqrs),
                "mvc" => Ok(Self::Mvc),
                _ => Err(
                    "invalid .xecute/architecture: expected cqrs or mvc; no files changed".into(),
                ),
            },
            // Projects created before architecture selection retain their service layout.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::Mvc),
            Err(error) => Err(format!("cannot read .xecute/architecture: {error}")),
        }
    }
}

fn names(input: &str) -> Result<(String, String)> {
    if input.is_empty()
        || !input
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        || !input.as_bytes()[0].is_ascii_lowercase()
    {
        return Err(
            "use a lowercase name starting with a letter, such as user or user-profile".into(),
        );
    }
    let snake = input.replace('-', "_");
    if snake.split('_').any(str::is_empty)
        || matches!(
            snake.as_str(),
            "self"
                | "super"
                | "crate"
                | "mod"
                | "type"
                | "fn"
                | "pub"
                | "use"
                | "impl"
                | "struct"
                | "enum"
                | "trait"
                | "async"
                | "await"
                | "loop"
                | "match"
                | "move"
                | "ref"
                | "let"
                | "const"
                | "static"
                | "where"
                | "in"
                | "for"
                | "if"
                | "else"
                | "return"
                | "break"
                | "continue"
                | "true"
                | "false"
                | "unsafe"
                | "extern"
                | "dyn"
                | "as"
                | "mut"
                | "while"
                | "union"
                | "abstract"
                | "become"
                | "box"
                | "do"
                | "final"
                | "macro"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "yield"
                | "try"
                | "gen"
        )
    {
        return Err("name must be a valid non-keyword Rust module name".into());
    }
    let pascal = snake
        .split('_')
        .map(|s| format!("{}{}", s[..1].to_uppercase(), &s[1..]))
        .collect();
    Ok((snake, pascal))
}

fn write(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))
}

fn format_files(root: &Path, paths: &[String]) -> Result<()> {
    let status = Command::new("rustfmt")
        .current_dir(root)
        .args(["--edition", "2024", "--config", "skip_children=true"])
        .args(paths)
        .status()
        .map_err(|error| {
            format!("files generated but formatting failed: {error}; run cargo fmt")
        })?;
    if !status.success() {
        return Err("files generated but formatting failed; run cargo fmt".into());
    }
    Ok(())
}

pub fn new_project(
    destination: &str,
    runtime: Option<&Path>,
    architecture: Architecture,
    name_override: Option<&str>,
) -> Result<()> {
    scaffold(destination, runtime, architecture, name_override, false)
}

fn scaffold(
    destination: &str,
    runtime: Option<&Path>,
    architecture: Architecture,
    name_override: Option<&str>,
    member: bool,
) -> Result<()> {
    let in_place = destination == ".";
    let root = if in_place {
        std::env::current_dir().map_err(|e| e.to_string())?
    } else {
        if !member {
            names(destination)?;
        }
        PathBuf::from(destination)
    };
    let derived_name = if in_place {
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_owned()
    } else {
        destination.to_owned()
    };
    let name = name_override.unwrap_or(&derived_name);
    names(name).map_err(|error| {
        format!("{error}; use --name <project-name> to choose the package name")
    })?;
    if !in_place && root.exists() {
        return Err(format!(
            "{} already exists; use xecute new . inside that directory",
            root.display()
        ));
    }
    let dependency = if let Some(runtime) = runtime {
        let runtime = runtime
            .canonicalize()
            .map_err(|e| format!("runtime path: {e}; supply --runtime-path"))?;
        if !runtime.join("Cargo.toml").is_file() {
            return Err("runtime path must contain xecute/Cargo.toml".into());
        }
        let runtime = runtime
            .to_str()
            .ok_or("runtime path must be UTF-8")?
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        format!("xecute = {{ path = \"{runtime}\" }}")
    } else {
        "xecute = { version = \"0.1.8\" }".to_owned()
    };
    let test_dependency = crate::manifest::testing_dependency(&dependency)?;
    let templates = [
        (
            "src/tests/versions.rs",
            include_str!("../templates/tests/versions.rs"),
        ),
        (
            "src/server/versions.rs",
            include_str!("../templates/server/versions.rs"),
        ),
        ("src/modules/mod.rs", "//! Application feature modules.\n"),
        (
            "src/tests/application.rs",
            include_str!("../templates/tests/application.rs"),
        ),
        (
            "src/tests/static_files.rs",
            include_str!("../templates/tests/static_files.rs"),
        ),
        (
            "src/configs/static_files.rs",
            include_str!("../templates/configs/static_files.rs"),
        ),
        (
            "src/server/static_files.rs",
            include_str!("../templates/server/static_files.rs"),
        ),
        (
            "public/index.html",
            "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Xecute static files</title><h1>Static file serving is working</h1><p>Edit public/index.html or add files to public/.</p><a href=\"/static/hello.txt\">Example text file</a></html>\n",
        ),
        ("public/hello.txt", "Static file serving is working.\n"),
        ("src/main.rs", include_str!("../templates/main.rs")),
        (
            "src/middleware.rs",
            include_str!("../templates/middleware.rs"),
        ),
        (
            "src/tests/docs.rs",
            include_str!("../templates/tests/docs.rs"),
        ),
        (
            "src/configs/docs.rs",
            include_str!("../templates/configs/docs.rs"),
        ),
        (
            "src/server/docs.rs",
            include_str!("../templates/server/docs.rs"),
        ),
        (
            "src/configs/environment.rs",
            include_str!("../templates/configs/environment.rs"),
        ),
        (
            "src/configs/mod.rs",
            include_str!("../templates/configs/mod.rs"),
        ),
        (
            "src/configs/server.rs",
            include_str!("../templates/configs/server.rs"),
        ),
        (
            "src/server/mod.rs",
            include_str!("../templates/server/mod.rs"),
        ),
        (
            "src/server/bootstrap.rs",
            include_str!("../templates/server/bootstrap.rs"),
        ),
        (
            "src/server/error.rs",
            include_str!("../templates/server/error.rs"),
        ),
        (
            "src/server/health.rs",
            include_str!("../templates/server/health.rs"),
        ),
        (
            "src/server/rate_limit.rs",
            include_str!("../templates/server/rate_limit.rs"),
        ),
        (
            "src/server/router.rs",
            include_str!("../templates/server/router.rs"),
        ),
        (
            "src/server/shutdown.rs",
            include_str!("../templates/server/shutdown.rs"),
        ),
        (
            "src/server/logging.rs",
            include_str!("../templates/server/logging.rs"),
        ),
        (
            "src/server/middleware/mod.rs",
            include_str!("../templates/server/middleware/mod.rs"),
        ),
        (
            "src/server/middleware/guard.rs",
            include_str!("../templates/server/middleware/guard.rs"),
        ),
        (
            "src/server/middleware/request_log.rs",
            include_str!("../templates/server/middleware/request_log.rs"),
        ),
        (
            "src/server/middleware/response.rs",
            include_str!("../templates/server/middleware/response.rs"),
        ),
        (
            "src/tests/mod.rs",
            include_str!("../templates/tests/mod.rs"),
        ),
        (
            "src/tests/config.rs",
            include_str!("../templates/tests/config.rs"),
        ),
        (
            "src/tests/logging.rs",
            include_str!("../templates/tests/logging.rs"),
        ),
        (
            "src/tests/rate_limit.rs",
            include_str!("../templates/tests/rate_limit.rs"),
        ),
        (
            "src/tests/server.rs",
            include_str!("../templates/tests/server.rs"),
        ),
        (
            "src/tests/support.rs",
            include_str!("../templates/tests/support.rs"),
        ),
        (
            "src/configs/logging.rs",
            include_str!("../templates/configs/logging.rs"),
        ),
        (
            "src/configs/cors.rs",
            include_str!("../templates/configs/cors.rs"),
        ),
        (".env.example", include_str!("../templates/env.example")),
        (
            "config/AppConfig.toml",
            include_str!("../templates/config/AppConfig.toml"),
        ),
        (
            "rust-toolchain.toml",
            include_str!("../templates/rust-toolchain.toml"),
        ),
        ("rustfmt.toml", include_str!("../templates/rustfmt.toml")),
        ("clippy.toml", include_str!("../templates/clippy.toml")),
        (".editorconfig", include_str!("../templates/editorconfig")),
        (".dockerignore", include_str!("../templates/dockerignore")),
        ("Dockerfile", include_str!("../templates/Dockerfile")),
        (
            ".github/workflows/ci.yml",
            include_str!("../templates/ci.yml"),
        ),
        ("deny.toml", include_str!("../templates/deny.toml")),
        (".gitattributes", include_str!("../templates/gitattributes")),
        (
            ".github/dependabot.yml",
            include_str!("../templates/dependabot.yml"),
        ),
    ];
    if in_place {
        let mut outputs: Vec<&str> = templates.iter().map(|(path, _)| *path).collect();
        outputs.extend([
            "Cargo.toml",
            ".xecute",
            ".gitignore",
            "README.md",
            "src/app_module.rs",
            "src/modules/example",
            "src/modules/mod.rs",
        ]);
        if runtime.is_none() {
            outputs.push("Cargo.lock");
        }
        // Validate every destination before writing anything. Symlinked parents
        // are rejected so initialization cannot write outside the chosen root.
        for output in outputs {
            let path = root.join(output);
            if path.symlink_metadata().is_ok() {
                return Err(format!(
                    "{} already exists; no files changed",
                    path.display()
                ));
            }
            let mut parent = path.parent();
            while let Some(path) = parent.filter(|path| *path != root) {
                if let Ok(metadata) = path.symlink_metadata()
                    && (!metadata.is_dir() || metadata.file_type().is_symlink())
                {
                    return Err(format!(
                        "{} must be a real directory; no files changed",
                        path.display()
                    ));
                }
                parent = path.parent();
            }
        }
    }
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let manifest = include_str!("../templates/Cargo.toml.in")
        .replace("__PROJECT_NAME__", name)
        .replace("__RUNTIME_DEPENDENCY__", &dependency)
        .replace("__RUNTIME_TEST_DEPENDENCY__", &test_dependency);
    let manifest = if member {
        let mut doc = manifest
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| e.to_string())?;
        doc.remove("workspace");
        doc.remove("profile");
        doc.to_string()
    } else {
        manifest
    };
    write(&root.join("Cargo.toml"), &manifest)?;
    fs::create_dir(root.join(".xecute")).map_err(|e| e.to_string())?;
    write(
        &root.join(".xecute/architecture"),
        &format!("{}\n", architecture.name()),
    )?;
    if !member {
        write(
            &root.join(".gitignore"),
            "/target/\n.env\n.env.*\n!.env.example\n/logs/\n/storage/\n",
        )?;
    }
    let mut formatted = Vec::new();
    for (path, template) in templates {
        if member
            && (path.starts_with(".github/")
                || WORKSPACE_FILES.iter().any(|(shared, _)| *shared == path))
        {
            continue;
        }
        if path.ends_with(".rs") {
            formatted.push(path.to_owned());
        }
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("template parent")).map_err(|e| e.to_string())?;
        write(&path, &template.replace("__PROJECT_NAME__", name))?;
    }
    write(
        &root.join("src/app_module.rs"),
        include_str!("../templates/app_module.rs"),
    )?;
    write(
        &root.join("README.md"),
        &include_str!("../templates/readme.md")
            .replace("__PROJECT_NAME__", name)
            .replace("__ARCHITECTURE__", architecture.name()),
    )?;
    if runtime.is_none() && !member {
        write(
            &root.join("Cargo.lock"),
            &include_str!("../templates/project.lock").replace("__PROJECT_NAME__", name),
        )?;
    }
    formatted.push("src/app_module.rs".into());
    format_files(&root, &formatted)?;
    component_inner(&root, "resource", "example", !member)?;
    if member {
        println!("Created {name} ({})", architecture.name());
        return Ok(());
    }
    let cd = if in_place {
        String::new()
    } else {
        format!("  cd {destination}\n")
    };
    println!(
        "Created {name} ({})\n{cd}  xecute run dev\n  Swagger: http://127.0.0.1:3000/docs/",
        architecture.name()
    );
    Ok(())
}

/// Create a Cargo workspace with an initial application named after the project.
pub fn new_workspace(
    destination: &str,
    runtime: Option<&Path>,
    architecture: Architecture,
    name_override: Option<&str>,
) -> Result<()> {
    let root = if destination == "." {
        std::env::current_dir().map_err(|e| e.to_string())?
    } else {
        names(destination)?;
        let root = PathBuf::from(destination);
        if root.exists() {
            return Err("destination already exists; no files changed".into());
        }
        root
    };
    let name = name_override
        .or_else(|| root.file_name().and_then(|s| s.to_str()))
        .ok_or("use --name to choose the initial app name")?;
    names(name)?;
    // Check all owned destinations before creating anything in an existing directory.
    for entry in [
        "Cargo.toml",
        "Cargo.lock",
        "apps",
        ".xecute",
        "README.md",
        ".gitignore",
        ".github",
        "rust-toolchain.toml",
        "rustfmt.toml",
        "clippy.toml",
        "deny.toml",
        ".editorconfig",
        ".gitattributes",
        ".dockerignore",
    ] {
        if root.join(entry).symlink_metadata().is_ok() {
            return Err(format!("{entry} already exists; no files changed"));
        }
    }
    let runtime = runtime
        .map(|p| p.canonicalize().map_err(|e| e.to_string()))
        .transpose()?;
    if runtime
        .as_ref()
        .is_some_and(|p| !p.join("Cargo.toml").is_file())
    {
        return Err("runtime path must contain xecute/Cargo.toml".into());
    }
    fs::create_dir_all(root.join(".xecute")).map_err(|e| e.to_string())?;
    write(&root.join(".xecute/workspace"), "1\n")?;
    write(&root.join(".xecute/architecture"), architecture.name())?;
    if let Some(runtime) = &runtime {
        write(
            &root.join(".xecute/runtime-path"),
            runtime.to_str().ok_or("runtime path must be UTF-8")?,
        )?;
    }
    write(
        &root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"apps/*\"]\nresolver = \"3\"\n\n[profile.release]\nlto = \"thin\"\ncodegen-units = 1\nstrip = \"symbols\"\n",
    )?;
    write(
        &root.join(".gitignore"),
        "/target/\n.env\n.env.*\n!.env.example\n**/logs/\n**/storage/\n",
    )?;
    write(
        &root.join("README.md"),
        &format!(
            "# {name}\n\nAdd an application: `xecute g apps <app_name> [--mvc | --cqrs]`\n\nRun an application: `cd apps/{name} && xecute run dev`\n\nBuild all applications: `cargo build --workspace`\n"
        ),
    )?;
    add_app(&root, name, Some(architecture), runtime.as_deref())?;
    for (file, template) in WORKSPACE_FILES {
        let path = root.join(file);
        fs::create_dir_all(path.parent().expect("workspace file parent"))
            .map_err(|e| e.to_string())?;
        write(&path, template)?;
    }
    fs::create_dir_all(root.join(".github/workflows")).map_err(|e| e.to_string())?;
    if !root.join(".github/workflows/ci.yml").exists() {
        write(
            &root.join(".github/workflows/ci.yml"),
            "name: CI\non: [push, pull_request]\npermissions:\n  contents: read\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: rustup show active-toolchain\n      - run: cargo fmt --all -- --check\n      - run: cargo clippy --workspace --all-targets --all-features --locked -- -D warnings\n      - run: cargo test --workspace --all-targets --all-features --locked\n",
        )?;
    }
    Ok(())
}

/// Add an application to a Xecute workspace, inheriting its architecture by default.
pub fn add_app(
    root: &Path,
    name: &str,
    architecture: Option<Architecture>,
    runtime: Option<&Path>,
) -> Result<()> {
    names(name)?;
    if !root.join(".xecute/workspace").is_file() {
        return Err(
            "run xecute g apps from a workspace created with xecute new --workspace".into(),
        );
    }
    let manifest = fs::read_to_string(root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let manifest = manifest
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| e.to_string())?;
    let members = manifest
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(toml_edit::Item::as_array)
        .ok_or("workspace manifest must contain members = [\"apps/*\"]")?;
    if !members
        .iter()
        .any(|member| member.as_str() == Some("apps/*"))
    {
        return Err("workspace members must include apps/*; no files changed".into());
    }
    let architecture = match architecture {
        Some(architecture) => architecture,
        None => Architecture::read(root)?,
    };
    let runtime_override = runtime.is_some();
    let inherited = match fs::read_to_string(root.join(".xecute/runtime-path")) {
        Ok(path) => Some(PathBuf::from(path.trim())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot read workspace runtime path: {error}")),
    };
    let runtime = runtime.or(inherited.as_deref());
    let apps = root.join("apps");
    if apps
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir())
    {
        return Err("apps must be a real directory; no files changed".into());
    }
    let destination = apps.join(name);
    if destination.symlink_metadata().is_ok() {
        return Err("app already exists; no files changed".into());
    }
    fs::create_dir_all(&apps).map_err(|e| e.to_string())?;
    scaffold(
        destination.to_str().ok_or("app path must be UTF-8")?,
        runtime,
        architecture,
        Some(name),
        true,
    )?;
    centralize_dependencies(&destination, root, runtime_override)?;
    if runtime_override {
        let runtime = runtime
            .expect("explicit runtime")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        write(
            &root.join(".xecute/runtime-path"),
            runtime.to_str().ok_or("runtime path must be UTF-8")?,
        )?;
    }
    let docker = fs::read_to_string(destination.join("Dockerfile"))
        .map_err(|e| e.to_string())?
        .replace("COPY src ./src", "COPY . .")
        .replace(
            "COPY public ./public",
            &format!("COPY apps/{name}/public ./public"),
        )
        .replace(
            "COPY config ./config",
            &format!("COPY apps/{name}/config ./config"),
        )
        .replace(
            "cargo build --release",
            &format!("cargo build -p {name} --release"),
        );
    write(&destination.join("Dockerfile"), &docker)?;
    // Resolve the new member while retaining existing dependency versions.
    let status = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .stdout(std::process::Stdio::null())
        .current_dir(root)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(
            "app created, but workspace lockfile generation failed; run cargo generate-lockfile"
                .into(),
        );
    }
    println!(
        "Workspace app ready: {} ({}). Run from workspace root: docker build -f apps/{name}/Dockerfile .",
        destination.display(),
        architecture.name()
    );
    Ok(())
}

fn centralize_dependencies(app: &Path, root: &Path, runtime_override: bool) -> Result<()> {
    let app_path = app.join("Cargo.toml");
    let root_path = root.join("Cargo.toml");
    let app_manifest = fs::read_to_string(&app_path).map_err(|e| e.to_string())?;
    let root_manifest = fs::read_to_string(&root_path).map_err(|e| e.to_string())?;
    let (app_manifest, root_manifest) =
        crate::manifest::workspace_dependencies(&app_manifest, &root_manifest, runtime_override)?;
    write(&root_path, &root_manifest)?;
    write(&app_path, &app_manifest)
}

pub fn component(root: &Path, kind: &str, name: &str) -> Result<()> {
    component_inner(root, kind, name, true)
}

fn component_inner(root: &Path, kind: &str, name: &str, inherit_dependencies: bool) -> Result<()> {
    let kind = match kind {
        "resource" | "res" => "resource",
        "module" | "mo" => "module",
        "controller" | "co" => "controller",
        "service" | "s" => "service",
        _ => return Err("generator must be resource, module, controller, or service".into()),
    };
    let architecture = Architecture::read(root)?;
    let cqrs = kind == "resource" && architecture == Architecture::Cqrs;
    let (snake, pascal) = names(name)?;
    if matches!(
        snake.as_str(),
        "app_module" | "main" | "middleware" | "modules"
    ) {
        return Err("name conflicts with application files".into());
    }
    let main_path = root.join("src/main.rs");
    let app_path = root.join("src/app_module.rs");
    let mut main =
        fs::read_to_string(&main_path).map_err(|_| "run generate from a Xecute project root")?;
    let mut app = fs::read_to_string(&app_path).map_err(|_| "missing src/app_module.rs")?;
    let modules = root.join("src/modules");
    if let Ok(metadata) = modules.symlink_metadata()
        && (!metadata.is_dir() || metadata.file_type().is_symlink())
    {
        return Err("src/modules must be a real directory; no files changed".into());
    }
    let index = modules.join("mod.rs");
    if index
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err("src/modules/mod.rs must not be a symlink; no files changed".into());
    }
    let mut index_content = match fs::read_to_string(&index) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "//! Application feature modules.\n\n".into()
        }
        Err(error) => return Err(format!("cannot read src/modules/mod.rs: {error}")),
    };
    let folder = modules.join(&snake);
    if folder.exists() || root.join("src").join(&snake).exists() {
        return Err(format!(
            "{} already exists; no files changed",
            folder.display()
        ));
    }
    if kind == "resource" && root.join(format!("src/tests/{snake}.rs")).exists() {
        return Err("resource test already exists; no files changed".into());
    }
    let markers = [
        "// xecute:modules",
        "// xecute:imports",
        "// xecute:definitions",
        "// xecute:routes",
    ];
    if markers.iter().any(|m| app.matches(m).count() != 1) {
        return Err(
            "app_module.rs must have the registration markers from xecute new; no files changed"
                .into(),
        );
    }
    if main.contains(&format!("mod {snake};")) {
        return Err("module already declared; no files changed".into());
    }
    let service = if kind == "resource" {
        include_str!("../templates/resource/service.rs")
    } else {
        include_str!("../templates/resource/standalone_service.rs")
    }
    .replace("__TYPE__", &pascal)
    .replace("__FEATURE__", &snake);
    let controller = if cqrs {
        include_str!("../templates/cqrs/controller.rs")
    } else if kind == "resource" {
        include_str!("../templates/resource/controller.rs")
    } else {
        include_str!("../templates/resource/standalone_controller.rs")
    }
    .replace("__TYPE__", &pascal)
    .replace("__FEATURE__", &snake);
    let mut files: Vec<(String, String)> = Vec::new();
    let mut mods = format!("//! Public building blocks for the {snake} feature.\n\n");
    if !cqrs && matches!(kind, "resource" | "service") {
        files.push(("service.rs".into(), service));
        mods.push_str(&format!(
            "mod service;\npub use service::{pascal}Service;\n"
        ));
    }
    if matches!(kind, "resource" | "controller") {
        files.push(("controller.rs".into(), controller));
        mods.push_str(&format!(
            "mod controller;\npub use controller::{pascal}Controller;\n"
        ));
    }
    if matches!(kind, "resource" | "module") {
        let module = if cqrs {
            include_str!("../templates/cqrs/module.rs").replace("__TYPE__", &pascal)
        } else if kind == "resource" {
            include_str!("../templates/resource/module.rs").replace("__TYPE__", &pascal)
        } else {
            format!(
                "//! Dependency-injection boundary for the {snake} feature.\n//! Add injectable services and controllers to providers as the feature grows.\n\nuse xecute::module;\n\n/// Dependency-injection module.\n#[module(providers = [])]\npub struct {pascal}Module;\n"
            )
        };
        files.push(("module.rs".into(), module));
        mods.push_str(&format!("mod module;\npub use module::{pascal}Module;\n"));
        app = app
            .replace(
                markers[0],
                &format!(
                    "use crate::modules::{snake}::{pascal}Module;\n{}",
                    markers[0]
                ),
            )
            .replace(markers[1], &format!("{pascal}Module,\n    {}", markers[1]))
            .replace(
                markers[2],
                &format!("{pascal}Module::definition(),\n            {}", markers[2]),
            );
        if kind == "resource" {
            if app.contains("// xecute:api") {
                app = app.replace("// xecute:api", &format!("let operations = operations.into_iter().chain(crate::modules::{snake}::{pascal}Controller::api_operations()).collect::<Vec<_>>();\n        // xecute:api"));
            }
            let controller_container = if app.contains("fn router_with_container(") {
                "container.clone()"
            } else {
                "::std::sync::Arc::new(container.clone())"
            };
            app = app.replace(markers[3], &format!("let router = router.merge(crate::modules::{snake}::{pascal}Controller::scoped_router({controller_container}));\n        {}", markers[3]));
        }
    }
    if kind == "resource" {
        for (name, template) in [
            ("dto/mod.rs", include_str!("../templates/resource/dto.rs")),
            (
                "entity/mod.rs",
                include_str!("../templates/resource/entity.rs"),
            ),
            (
                "repository/store.rs",
                include_str!("../templates/resource/repository.rs"),
            ),
        ] {
            files.push((
                name.into(),
                template
                    .replace("__TYPE__", &pascal)
                    .replace("__FEATURE__", &snake),
            ));
        }
        files.push(("repository/mod.rs".into(), format!("//! Persistence adapters for this feature.\n\nmod store;\npub use store::{{{pascal}Repository, CapacityExceeded}};\n")));
        mods.push_str("\npub mod dto;\npub mod entity;\npub mod repository;\n");
    }
    if cqrs {
        for (path, template) in [
            (
                "commands/create.rs",
                include_str!("../templates/cqrs/create.rs"),
            ),
            ("queries/list.rs", include_str!("../templates/cqrs/list.rs")),
        ] {
            files.push((path.into(), template.replace("__TYPE__", &pascal)));
        }
        files.push(("commands/mod.rs".into(), format!("//! Write operations.\nmod create;\npub use create::{{Create{pascal}Command, Create{pascal}Handler}};\n")));
        files.push(("queries/mod.rs".into(), format!("//! Read operations.\nmod list;\npub use list::{{List{pascal}Query, List{pascal}Handler}};\n")));
        files.push((
            format!("services/{snake}_service.rs"),
            include_str!("../templates/cqrs/service.rs").replace("__TYPE__", &pascal),
        ));
        files.push(("services/mod.rs".into(), format!("//! Shared business services for this feature.\nmod {snake}_service;\npub use {snake}_service::{pascal}Service;\n")));
        mods.push_str("\npub mod commands;\npub mod queries;\npub mod services;\n");
    }
    if kind == "resource" {
        files.push((
            "tests/http.rs".into(),
            include_str!("../templates/resource/tests.rs").replace("__FEATURE__", &snake),
        ));
        files.push((
            "tests/mod.rs".into(),
            "//! Feature HTTP integration tests.\nmod http;\n".into(),
        ));
        mods.push_str("\n#[cfg(test)]\nmod tests;\n");
    }
    files.push(("mod.rs".into(), mods));
    if index_content.contains(&format!("mod {snake};")) {
        return Err("module already declared in src/modules/mod.rs; no files changed".into());
    }
    index_content.push_str(&format!("pub mod {snake};\n"));
    let declaration = "pub mod modules;\n";
    if !main.contains("mod modules;") {
        if let Some(index) = main
            .find("pub mod app_module;")
            .or_else(|| main.find("mod app_module;"))
        {
            main.insert_str(index, declaration);
        } else {
            main.push_str(&format!("\n{declaration}"));
        }
    }
    if kind == "resource" {
        let manifest_path = root.join("Cargo.toml");
        if manifest_path.exists() {
            let before = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
            let manifest = crate::manifest::resource_dependencies(&before)?;
            let workspace_root = root
                .ancestors()
                .skip(1)
                .filter(|_| inherit_dependencies)
                .find(|parent| parent.join(".xecute/workspace").is_file());
            let manifest = if let Some(workspace_root) = workspace_root {
                let workspace_path = workspace_root.join("Cargo.toml");
                let before_workspace =
                    fs::read_to_string(&workspace_path).map_err(|e| e.to_string())?;
                let (manifest, workspace) =
                    crate::manifest::workspace_dependencies(&manifest, &before_workspace, false)?;
                if workspace != before_workspace {
                    write(&workspace_path, &workspace)?;
                }
                manifest
            } else {
                manifest
            };
            if before != manifest {
                write(&manifest_path, &manifest)?;
                println!("Updated resource dependencies; cargo build will refresh Cargo.lock.");
            }
        }
    }
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut formatted = vec![
        "src/main.rs".into(),
        "src/app_module.rs".into(),
        "src/modules/mod.rs".into(),
    ];
    for (name, content) in files {
        formatted.push(format!("src/modules/{snake}/{name}"));
        let path = folder.join(name);
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        write(&path, &content)?;
    }
    write(&index, &index_content)?;
    write(&main_path, &main)?;
    write(&app_path, &app)?;
    format_files(root, &formatted)?;
    println!("Created {kind} {pascal} in src/modules/{snake}");
    if matches!(kind, "service" | "controller") {
        println!("Add this building block to a module's providers when ready to use it.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_reject_paths_and_keywords() {
        for name in ["../user", "user/name", "mod", "user__name", "1user"] {
            assert!(names(name).is_err());
        }
        assert_eq!(
            names("user-profile").unwrap(),
            ("user_profile".into(), "UserProfile".into())
        );
    }
    #[test]
    fn resource_registers_and_duplicate_preserves_files() {
        let root = std::env::temp_dir().join(format!("xecute-cli-test-{}", std::process::id()));
        fs::create_dir_all(root.join("src")).unwrap();
        write(&root.join("src/main.rs"), "mod app_module;\n").unwrap();
        write(
            &root.join("src/app_module.rs"),
            include_str!("../templates/app_module.rs"),
        )
        .unwrap();
        component(&root, "res", "user").unwrap();
        let before = fs::read_to_string(root.join("src/app_module.rs")).unwrap();
        assert!(before.contains("UserController::scoped_router"));
        assert!(component(&root, "res", "user").is_err());
        assert_eq!(
            before,
            fs::read_to_string(root.join("src/app_module.rs")).unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
