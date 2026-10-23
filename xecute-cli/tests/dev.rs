//! Exercise real Cargo rebuilds, failure recovery, ignored output, and signal cleanup.
#![cfg(unix)]

use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use std::{
    fs,
    io::Write,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Session {
    child: Child,
    root: PathBuf,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = kill(Pid::from_raw(self.child.id() as i32), Signal::SIGTERM);
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn wait_for(mut check: impl FnMut() -> bool, label: &str, log: &PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !check() {
        assert!(
            Instant::now() < deadline,
            "timed out: {label}\n{}",
            fs::read_to_string(log).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(50));
    }
}
fn source(version: &str) -> String {
    format!(
        r#"fn main() {{
        use std::io::Write;
        let listener = std::net::TcpListener::bind(std::env::var("DEV_TEST_ADDRESS").unwrap()).unwrap();
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open("starts.txt").unwrap();
        writeln!(file, "{version}|{{}}|{{}}", std::process::id(), std::env::args().nth(1).unwrap()).unwrap();
        for stream in listener.incoming() {{ drop(stream); }}
    }}"#
    )
}

#[test]
fn dev_restarts_recovers_and_releases_the_server_port_on_ctrl_c() {
    let root = std::env::temp_dir().join(format!(
        "xecute-dev-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("external")).unwrap();
    let external = root.join("external/extra.txt");
    fs::write(&external, "initial").unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dev-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), source("first")).unwrap();
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    drop(socket);
    let log = root.join("dev.log");
    let output = fs::File::create(&log).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_xecute"))
        .args(["run", "dev", "--watch"])
        .arg(&external)
        .args(["--", "argument with spaces"])
        .current_dir(&root)
        .env("DEV_TEST_ADDRESS", address.to_string())
        .env("CARGO_TARGET_DIR", root.join("compiled"))
        .stdout(Stdio::null())
        .stderr(output)
        .spawn()
        .unwrap();
    let mut session = Session {
        child,
        root: root.clone(),
    };
    let starts = root.join("starts.txt");
    let count = || {
        fs::read_to_string(&starts)
            .unwrap_or_default()
            .lines()
            .count()
    };
    wait_for(|| count() == 1, "initial server start", &log);
    assert!(fs::read_to_string(&starts).unwrap().contains("first|"));
    assert!(
        fs::read_to_string(&starts)
            .unwrap()
            .contains("argument with spaces")
    );
    fs::write(root.join("compiled/ignored.rs"), "not source").unwrap();
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(
        count(),
        1,
        "custom target output caused a rebuild: {}",
        fs::read_to_string(&log).unwrap()
    );
    fs::write(root.join("src/main.rs"), source("second")).unwrap();
    wait_for(|| count() == 2, "source restart", &log);
    assert!(fs::read_to_string(&starts).unwrap().contains("second|"));
    fs::write(root.join("src/main.rs"), "this is a compile error").unwrap();
    wait_for(
        || {
            fs::read_to_string(&log)
                .unwrap_or_default()
                .contains("Process exited")
        },
        "build failure stays in watch mode",
        &log,
    );
    assert!(session.child.try_wait().unwrap().is_none());
    fs::write(root.join("src/main.rs"), source("recovered")).unwrap();
    wait_for(|| count() == 3, "recovery from compile error", &log);
    fs::write(root.join(".env"), "EXAMPLE=changed\n").unwrap();
    wait_for(|| count() == 4, "environment restart", &log);
    fs::OpenOptions::new()
        .append(true)
        .open(external)
        .unwrap()
        .write_all(b" changed")
        .unwrap();
    wait_for(|| count() == 5, "explicit extra file restart", &log);
    kill(Pid::from_raw(session.child.id() as i32), Signal::SIGINT).unwrap();
    wait_for(
        || session.child.try_wait().unwrap().is_some(),
        "Ctrl-C shutdown",
        &log,
    );
    assert!(
        TcpListener::bind(address).is_ok(),
        "server process left its port occupied"
    );
}
