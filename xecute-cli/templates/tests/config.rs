//! Configuration boundary tests: invalid values must fail instead of weaken limits.

use crate::configs::Config;
#[test]
fn rejects_invalid_configuration() {
    for (key, value) in [
        ("RATE_LIMIT_REQUESTS", "0"),
        ("MAX_IN_FLIGHT_REQUESTS", "bad"),
        ("BIND_ADDRESS", "bad"),
        ("CORS_ORIGINS", "*"),
        ("CORS_ORIGINS", r#"["https://example.com/private"]"#),
        ("CORS_ORIGINS", r#"["*"]"#),
        ("CORS_ALLOWED_METHODS", r#"["get"]"#),
        ("CORS_ALLOWED_HEADERS", r#"["bad header"]"#),
        ("CORS_EXPOSE_HEADERS", r#"["*"]"#),
        ("CORS_ALLOW_CREDENTIALS", "true"),
        ("CORS_MAX_AGE_SECONDS", "86401"),
    ] {
        assert!(Config::from_lookup(|k| Ok((k == key).then(|| value.to_owned()))).is_err());
    }
    let config = Config::from_lookup(|k| {
        Ok((k == "CORS_ORIGINS")
            .then(|| r#"["https://example.com","http://localhost:5173"]"#.into()))
    })
    .unwrap();
    assert_eq!(config.cors.origins.len(), 2);
}
