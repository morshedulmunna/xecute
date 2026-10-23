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
