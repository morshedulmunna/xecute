//! TOML startup validation and separation between ordinary settings and secrets.

use crate::configs::{Config, ConfigFile};

#[test]
fn rejects_invalid_configuration() {
    for source in [
        "[rate_limit]\nrequests = 0",
        "[server]\nmax_in_flight_requests = 'bad'",
        "[server]\nbind_address = 'bad'",
        "[server]\nbody_limit_bytes = -1",
        "[server]\nrequest_timeout_seconds = '30'",
        "[cors]\norigins = '*'",
        "[cors]\norigins = ['https://example.com/private']",
        "[cors]\norigins = ['*']",
        "[cors]\nallowed_methods = ['get']",
        "[cors]\nallowed_headers = ['bad header']",
        "[cors]\nexposed_headers = ['*']",
        "[cors]\nallow_credentials = true",
        "[cors]\nmax_age_seconds = 86401",
        "[docs]\nenabled = 'false'",
        "[server]\nbind_adress = '127.0.0.1:3000'",
        "[unknown_section]",
        "[logging]\nstdout_enabled = 1",
        "[api]\nversioning_enabled = 'true'",
    ] {
        assert!(Config::from_config(&ConfigFile::parse(source).unwrap()).is_err(), "{source}");
    }
    let config = Config::from_config(&ConfigFile::parse("[cors]\norigins = ['https://example.com', 'http://localhost:5173']").unwrap()).unwrap();
    assert_eq!(config.cors.origins.len(), 2);
}

#[test]
fn generated_file_contains_normal_settings_and_env_example_contains_only_secrets() {
    let config = Config::from_config(&ConfigFile::parse(include_str!("../../config/AppConfig.toml")).unwrap()).unwrap();
    assert_eq!(config.bind_address.to_string(), "127.0.0.1:3000");
    assert!(config.logging.file_enabled);
    assert!(config.docs.enabled);
    assert!(include_str!("../../.env.example").lines().all(|line| line.trim().is_empty() || line.starts_with('#')));
    assert!(Config::from_path("missing-AppConfig.toml").is_err());
}
