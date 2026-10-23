//! Log contract tests: one flat event per request and no private header or query values.

use super::support::{app, request};
use crate::configs::Config;
use axum::http::{HeaderValue, header};
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Clone, Default)]
struct LogCapture(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogCapture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn logs_one_flat_completion_per_request_and_omits_private_request_values() {
    // Isolate tracing's process-wide callsite cache from concurrent HTTP tests.
    if std::env::var_os("XECUTE_LOG_TEST_CHILD").is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let test = format!(
            "{module}::logs_one_flat_completion_per_request_and_omits_private_request_values"
        );
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &test, "--test-threads=1"])
                .env("XECUTE_LOG_TEST_CHILD", "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    use tracing::instrument::WithSubscriber;
    let capture = LogCapture::default();
    let subscriber = crate::server::logging::subscriber(
        tracing_subscriber::EnvFilter::new("info"),
        capture.clone(),
    );
    let responses = async {
        let app = app(&Config::default());
        let mut responses = Vec::new();
        for path in ["/items?token=private-query", "/unknown?token=private-query"] {
            let mut req = request(path, "127.0.0.1");
            req.headers_mut().insert(
                header::AUTHORIZATION,
                HeaderValue::from_static("Bearer private-header"),
            );
            responses.push(app.clone().oneshot(req).await.unwrap());
        }
        responses
    }
    .with_subscriber(subscriber)
    .await;
    let bytes = capture.0.lock().unwrap().clone();
    let text = String::from_utf8(bytes).unwrap();
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 2);
    for (index, (event, response)) in events.iter().zip(responses.iter()).enumerate() {
        assert_eq!(event["message"], "request completed");
        assert_eq!(event["level"], "INFO");
        assert_eq!(event["status"], if index == 0 { 200 } else { 404 });
        assert!(event["duration_ms"].is_number());
        assert!(event["timestamp"].is_string());
        assert_eq!(
            event["request_id"],
            response.headers()["x-request-id"].to_str().unwrap()
        );
        assert_eq!(event["method"], "GET");
        assert_eq!(
            event["route"],
            if index == 0 { "/items" } else { "/unknown" }
        );
        assert!(event["response_bytes"].is_number());
        for field in ["fields", "span", "spans", "target"] {
            assert!(event.get(field).is_none());
        }
    }
    assert!(!text.contains("private-query"));
    assert!(!text.contains("private-header"));
}

#[test]
fn file_output_flushes_and_accepts_an_additional_exporter_layer() {
    if std::env::var_os("XECUTE_FILE_LOG_TEST_CHILD").is_none() {
        let directory = std::env::temp_dir().join(format!("xecute-log-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let module = module_path!().split_once("::").unwrap().1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("{module}::file_output_flushes_and_accepts_an_additional_exporter_layer"), "--test-threads=1"])
            .env("XECUTE_FILE_LOG_TEST_CHILD", "1").env("RUST_LOG", "info")
            .env("LOG_STDOUT_ENABLED", "false")
            .env("LOG_FILE_DIR", &directory).status().unwrap();
        assert!(status.success());
        let path = std::fs::read_dir(&directory).unwrap().next().unwrap().unwrap().path();
        let content = std::fs::read_to_string(path).unwrap();
        let events: Vec<serde_json::Value> = content.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["message"], "file and exporter event");
        assert_eq!(events[0]["answer"], 42);
        assert!(events[0].get("fields").is_none());
        std::fs::remove_dir_all(directory).unwrap();
        return;
    }
    struct Exporter(Arc<std::sync::atomic::AtomicUsize>);
    impl tracing_subscriber::Layer<tracing_subscriber::Registry> for Exporter {
        fn on_event(&self, _: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, tracing_subscriber::Registry>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let guard = crate::server::logging::init_with_layer(&Config::default(), Exporter(count.clone())).unwrap();
    tracing::info!(answer = 42, "file and exporter event");
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    drop(guard);
}

#[test]
fn logger_config_uses_two_settings() {
    use crate::configs::LoggingConfig;
    assert!(LoggingConfig::from_lookup(|_| Ok(None)).unwrap().stdout);
    assert!(!LoggingConfig::from_lookup(|_| Ok(None)).unwrap().file_enabled);
    assert!(!LoggingConfig::from_lookup(|key| Ok((key == "LOG_FILE_DIR").then(String::new))).unwrap().file_enabled);
    let config = LoggingConfig::from_lookup(|key| Ok(match key {
        "LOG_STDOUT_ENABLED" => Some("false".into()),
        "LOG_FILE_DIR" => Some("logs".into()),
        _ => None,
    })).unwrap();
    assert!(!config.stdout);
    assert!(config.file_enabled);
    assert_eq!(config.filename, "app");
    assert_eq!(config.rotation, "daily");
    assert_eq!(config.max_files, 7);
    assert!(LoggingConfig::from_lookup(|key| Ok((key == "LOG_STDOUT_ENABLED").then(|| "invalid".into()))).is_err());
}

#[test]
fn public_log_destinations_are_rejected_before_creating_files() {
    let root = std::env::temp_dir().join(format!("xecute-private-log-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let config = Config { static_files: crate::configs::StaticConfig { directory: root.clone(), ..Default::default() }, ..Default::default() };
    let logging = crate::configs::LoggingConfig::from_lookup(|key| Ok((key == "LOG_FILE_DIR").then(|| root.join("logs").to_string_lossy().into_owned()))).unwrap();
    assert!(crate::server::logging::validate_output_paths(&config, &logging).is_err());
    assert!(!root.join("logs").exists());
    std::fs::remove_dir_all(root).unwrap();
}
