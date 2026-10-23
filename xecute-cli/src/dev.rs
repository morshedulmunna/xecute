//! Development rebuild/restart loop with bounded process-tree shutdown.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

type Result<T> = std::result::Result<T, String>;
const POLL: Duration = Duration::from_millis(200);
const DEBOUNCE: Duration = Duration::from_millis(300);
#[cfg(unix)]
const SHUTDOWN: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct Options {
    binary: Option<String>,
    features: Vec<String>,
    watch: Vec<PathBuf>,
    arguments: Vec<String>,
}

impl Options {
    pub fn parse(arguments: &[String]) -> Result<Self> {
        let mut options = Self::default();
        let mut arguments = arguments.iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--" => {
                    options.arguments.extend(arguments.cloned());
                    break;
                }
                "--bin" if options.binary.is_none() => {
                    let value = arguments
                        .next()
                        .filter(|v| !v.starts_with('-') && !v.is_empty())
                        .ok_or("--bin requires a binary name")?;
                    options.binary = Some(value.clone());
                }
                "--features" => {
                    let value = arguments
                        .next()
                        .filter(|value| !value.starts_with('-') && !value.trim().is_empty())
                        .ok_or("--features requires a Cargo feature list")?;
                    options.features.push(value.clone());
                }
                "--watch" => {
                    let value = arguments
                        .next()
                        .filter(|v| !v.starts_with('-') && !v.is_empty())
                        .ok_or("--watch requires a path")?;
                    options.watch.push(PathBuf::from(value));
                }
                _ => return Err(format!("unknown or duplicate dev option: {argument}")),
            }
        }
        Ok(options)
    }
}

// Poll metadata rather than requiring an external watcher binary or OS watch limits.
// Store paths so additions, deletions, and editor rename-save operations are detected.
type Snapshot = BTreeMap<PathBuf, (u64, SystemTime)>;

fn ignored_directory(name: &str) -> bool {
    matches!(
        name,
        "target"
            | ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | "dist"
            | "build"
            | ".idea"
            | ".vscode"
    )
}

fn watched_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name == ".env"
        || name.starts_with(".env.")
        || name == "Cargo.lock"
        || matches!(
            path.extension().and_then(|s| s.to_str()),
            Some(
                "rs" | "toml"
                    | "json"
                    | "yaml"
                    | "yml"
                    | "sql"
                    | "html"
                    | "css"
                    | "js"
                    | "ts"
                    | "graphql"
            )
        )
}

fn snapshot(roots: &[PathBuf], target: &Path) -> Result<Snapshot> {
    fn visit(path: &Path, files: &mut Snapshot, explicit: bool, target: &Path) -> Result<()> {
        if path.starts_with(target) {
            return Ok(());
        }
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("cannot watch {}: {error}", path.display())),
        };
        // Avoid following directory symlinks into cycles or unrelated trees.
        if metadata.file_type().is_symlink() {
            return Ok(());
        }
        if metadata.is_dir() {
            for entry in
                fs::read_dir(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?
            {
                let entry = entry.map_err(|e| e.to_string())?;
                if !ignored_directory(&entry.file_name().to_string_lossy()) {
                    visit(&entry.path(), files, false, target)?;
                }
            }
        } else if metadata.is_file() && (explicit || watched_file(path)) {
            files.insert(
                path.to_owned(),
                (
                    metadata.len(),
                    metadata.modified().map_err(|e| e.to_string())?,
                ),
            );
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    for root in roots {
        visit(root, &mut files, true, target)?;
    }
    Ok(files)
}

// Resolve symlinked ancestors even before Cargo creates the target directory.
fn canonical_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path.canonicalize().map_err(|e| e.to_string());
    }
    let parent = path.parent().ok_or("target directory has no parent")?;
    let name = path.file_name().ok_or("target directory has no name")?;
    Ok(canonical_path(parent)?.join(name))
}

struct Process(Child);

impl Process {
    fn start(root: &Path, options: &Options) -> Result<Self> {
        let mut command = Command::new("cargo");
        command.current_dir(root).arg("run");
        if let Some(binary) = &options.binary {
            command.args(["--bin", binary]);
        }
        for features in &options.features {
            command.args(["--features", features]);
        }
        if !options.arguments.is_empty() {
            command.arg("--").args(&options.arguments);
        }
        command.stdin(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Cargo, rustc, the server, and its ordinary children share this group.
            command.process_group(0);
        }
        command
            .spawn()
            .map(Self)
            .map_err(|e| format!("could not start cargo run: {e}"))
    }

    fn stop(&mut self) {
        #[cfg(unix)]
        {
            use nix::{
                sys::signal::{Signal, killpg},
                unistd::Pid,
            };
            let group = Pid::from_raw(self.0.id() as i32);
            let _ = killpg(group, Signal::SIGTERM);
            let deadline = Instant::now() + SHUTDOWN;
            loop {
                let _ = self.0.try_wait();
                if killpg(group, None).is_err() {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = killpg(group, Signal::SIGKILL);
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
        #[cfg(windows)]
        {
            // Terminate Cargo and its child server together, rather than just Cargo.
            let _ = Command::new("taskkill")
                .args(["/PID", &self.0.id().to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn run(options: Options) -> Result<()> {
    let root = env::current_dir().map_err(|e| e.to_string())?;
    if !root.join("Cargo.toml").is_file() {
        return Err("run xecute run dev from a Rust project root containing Cargo.toml".into());
    }
    let mut roots = vec![root.clone()];
    for path in &options.watch {
        roots.push(
            path.canonicalize()
                .map_err(|e| format!("watch path {}: {e}", path.display()))?,
        );
    }
    let metadata = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("could not inspect Cargo project: {e}"))?;
    if !metadata.status.success() {
        return Err(format!(
            "could not inspect Cargo project:\n{}",
            String::from_utf8_lossy(&metadata.stderr)
        ));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).map_err(|e| e.to_string())?;
    let target = PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("Cargo metadata is missing target_directory")?,
    );
    let target = canonical_path(&target)?;
    let mut previous = snapshot(&roots, &target)?;
    let lock_path = root.join("Cargo.lock");
    let mut awaiting_initial_lock = !lock_path.exists();
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
    })
    .map_err(|e| format!("could not register shutdown handler: {e}"))?;
    eprintln!(
        "[xecute dev] Watching {} path(s). Save to rebuild/restart; Ctrl-C to stop.",
        roots.len()
    );
    let mut child = Some(Process::start(&root, &options)?);
    let mut changed = None;
    while running.load(Ordering::SeqCst) {
        thread::sleep(POLL);
        let current = snapshot(&roots, &target)?;
        // Cargo creates the initial lockfile itself. It is already part of the
        // in-progress build; do not cancel that build just to consume the same lock.
        if awaiting_initial_lock && let Some(stamp) = current.get(&lock_path) {
            previous.insert(lock_path.clone(), *stamp);
            awaiting_initial_lock = false;
        }
        if current != previous {
            if let Some(path) = current
                .iter()
                .find(|(path, stamp)| previous.get(*path) != Some(*stamp))
                .map(|(path, _)| path)
                .or_else(|| previous.keys().find(|path| !current.contains_key(*path)))
            {
                eprintln!("[xecute dev] File changed: {}", path.display());
            }
            previous = current;
            changed = Some(Instant::now());
        }
        if let Some(process) = &mut child
            && let Some(status) = process.0.try_wait().map_err(|e| e.to_string())?
        {
            eprintln!("[xecute dev] Process exited ({status}); waiting for changes.");
            child = None;
        }
        if changed.is_some_and(|time| time.elapsed() >= DEBOUNCE) {
            changed = None;
            // Drop stops/reaps the whole previous process tree before rebinding ports.
            drop(child.take());
            if !running.load(Ordering::SeqCst) {
                break;
            }
            eprintln!("[xecute dev] Change detected; rebuilding and restarting...");
            child = Some(Process::start(&root, &options)?);
        }
    }
    eprintln!("[xecute dev] Stopping...");
    drop(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_preserve_application_arguments_and_reject_invalid_flags() {
        let args = [
            "--bin",
            "server",
            "--watch",
            "../runtime",
            "--features",
            "s3",
            "--",
            "--port",
            "4000",
        ]
        .map(String::from);
        let options = Options::parse(&args).unwrap();
        assert_eq!(options.binary.as_deref(), Some("server"));
        assert_eq!(options.features, ["s3"]);
        assert_eq!(options.arguments, ["--port", "4000"]);
        assert_eq!(options.watch, [PathBuf::from("../runtime")]);
        for args in [
            vec!["--watch"],
            vec!["--features"],
            vec!["--bin"],
            vec!["--bin", "a", "--bin", "b"],
            vec!["--release"],
        ] {
            assert!(
                Options::parse(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err()
            );
        }
    }
}
