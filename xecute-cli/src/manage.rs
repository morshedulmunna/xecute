//! Manage the Cargo installation containing the running CLI.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

pub enum Action {
    Update,
    Uninstall,
}

fn installation_root(executable: &Path) -> Result<PathBuf, String> {
    let bin = executable
        .parent()
        .ok_or("cannot locate the executable directory")?;
    if bin.file_name().is_none_or(|name| name != "bin") {
        return Err("this executable is not Cargo-installed; install it with cargo install --path xecute-cli --locked --force first".into());
    }
    let root = bin
        .parent()
        .ok_or("cannot locate the Cargo installation root")?;
    let tracking = fs::read(root.join(".crates2.json"))
        .map_err(|_| "Cargo installation metadata not found; use cargo install/uninstall directly for this installation")?;
    let tracking: serde_json::Value = serde_json::from_slice(&tracking)
        .map_err(|e| format!("invalid Cargo installation metadata: {e}"))?;
    let registered = tracking["installs"].as_object().is_some_and(|installs| {
        installs.iter().any(|(package, metadata)| {
            package.starts_with("xecute-cli ")
                && metadata["bins"]
                    .as_array()
                    .is_some_and(|bins| bins.iter().any(|bin| bin.as_str() == Some("xecute")))
        })
    });
    if !registered {
        return Err(
            "xecute-cli is not registered in this Cargo installation; use Cargo directly".into(),
        );
    }
    root.canonicalize()
        .map_err(|e| format!("cannot resolve Cargo installation root: {e}"))
}

#[cfg(not(windows))]
fn cargo_arguments(action: &Action, root: &Path) -> Vec<std::ffi::OsString> {
    let mut arguments: Vec<std::ffi::OsString> = match action {
        Action::Update => [
            "install",
            "xecute-cli",
            "--registry",
            "crates-io",
            "--locked",
            "--force",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
        Action::Uninstall => ["uninstall", "xecute-cli"]
            .into_iter()
            .map(Into::into)
            .collect(),
    };
    arguments.push("--root".into());
    arguments.push(root.as_os_str().to_owned());
    arguments
}

pub fn run(action: Action) -> Result<(), String> {
    let executable = env::current_exe().map_err(|e| format!("cannot locate xecute: {e}"))?;
    let root = installation_root(&executable)?;
    let label = match action {
        Action::Update => "Updating",
        Action::Uninstall => "Uninstalling",
    };
    println!("{label} xecute-cli in {}...", root.display());
    #[cfg(windows)]
    {
        // The helper waits for this process to exit, releasing the executable's
        // Windows file lock before Cargo replaces or removes it. Paths are passed
        // through environment variables, never interpolated into script text.
        let script = match action {
            Action::Update => {
                "Wait-Process -Id $env:XECUTE_PARENT_PID -ErrorAction SilentlyContinue; & cargo install xecute-cli --registry crates-io --locked --force --root $env:XECUTE_INSTALL_ROOT; exit $LASTEXITCODE"
            }
            Action::Uninstall => {
                "Wait-Process -Id $env:XECUTE_PARENT_PID -ErrorAction SilentlyContinue; & cargo uninstall xecute-cli --root $env:XECUTE_INSTALL_ROOT; exit $LASTEXITCODE"
            }
        };
        Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", script])
            .env("XECUTE_PARENT_PID", std::process::id().to_string())
            .env("XECUTE_INSTALL_ROOT", &root)
            .current_dir(&root)
            .spawn()
            .map_err(|e| format!("could not launch Cargo helper: {e}"))?;
        println!(
            "Cargo will continue after xecute exits; its output will report completion or errors."
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("cargo")
            .args(cargo_arguments(&action, &root))
            .current_dir(&root)
            .status()
            .map_err(|e| format!("could not start Cargo: {e}"))?;
        if !status.success() {
            return Err(format!("Cargo failed ({status}); see its output above"));
        }
        let completed = match action {
            Action::Update => "Update",
            Action::Uninstall => "Uninstall",
        };
        println!("{completed} completed.");
        Ok(())
    }
}
