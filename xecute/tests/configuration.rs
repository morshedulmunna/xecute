//! Behavioral regression coverage for configuration, errors, and provider lifecycles.

use xecute::config::Env;

#[test]
fn typed_required_defaults_ranges_json_and_validation() {
    let source = |key: &str| {
        Ok(match key {
            "PORT" => Some("5432".into()),
            "ENABLED" => Some("false".into()),
            "ORIGINS" => Some(r#"["https://example.com"]"#.into()),
            "EMPTY" => Some("".into()),
            "SECRET" => Some("very-private-value".into()),
            _ => None,
        })
    };
    let env = Env::new(&source);
    assert_eq!(env.required::<u16>("PORT").unwrap(), 5432);
    assert!(!env.get_or("ENABLED", true).unwrap());
    assert_eq!(env.range("PORT", 80, 1, 65535).unwrap(), 5432);
    assert!(env.range("PORT", 80, 1, 100).is_err());
    assert_eq!(env.get_or("MISSING", 42_u32).unwrap(), 42);
    assert!(env.optional::<u32>("MISSING").unwrap().is_none());
    assert_eq!(
        env.optional::<String>("EMPTY").unwrap().as_deref(),
        Some("")
    );
    assert!(env.required::<String>("EMPTY").is_err());
    assert!(env.required::<String>("MISSING").is_err());
    let origins: Vec<String> = env.json("ORIGINS", Vec::new()).unwrap();
    assert_eq!(origins, ["https://example.com"]);
    assert_eq!(
        env.validate("PORT", 5432, |port| *port > 1024).unwrap(),
        5432
    );
    assert!(env.validate("PORT", 80, |port| *port > 1024).is_err());
    let secret = env.secret("SECRET").unwrap();
    assert_eq!(secret.expose(), "very-private-value");
    assert!(!format!("{secret:?}").contains("very-private-value"));
}

#[test]
fn supplied_invalid_values_and_source_failures_never_leak_values() {
    for raw in [
        "very-private-value",
        " ",
        "TRUE",
        "-1",
        "999999999999999999999999999999",
    ] {
        let source = |_: &str| Ok(Some(raw.into()));
        let env = Env::new(&source);
        let error = env.required::<u16>("PRIVATE_PORT").unwrap_err();
        assert_eq!(error.key(), "PRIVATE_PORT");
        if raw.trim().len() > 2 {
            assert!(!format!("{error:?}").contains(raw));
        }
        assert!(env.get_or("PRIVATE_PORT", 80_u16).is_err());
        assert!(env.json::<Vec<String>>("PRIVATE_PORT", vec![]).is_err());
    }
    let source = |_: &str| Err("secret from custom source".into());
    let error = Env::new(&source).required::<String>("TOKEN").unwrap_err();
    assert!(!error.to_string().contains("secret"));
    assert!(error.to_string().contains("TOKEN"));
}

#[test]
fn toml_settings_preserve_types_defaults_and_validate_known_keys() {
    use xecute::config::ConfigFile;
    let file = ConfigFile::parse("[server]\nport = 3000\n[logging]\nenabled = false\n[cors]\norigins = ['https://example.com']").unwrap();
    assert_eq!(file.required::<u16>("server.port").unwrap(), 3000);
    assert!(!file.get_or("logging.enabled", true).unwrap());
    assert_eq!(file.range("server.port", 80, 1, 65535).unwrap(), 3000);
    assert!(file.range("server.port", 80, 1, 1024).is_err());
    assert!(file.optional::<String>("server.port").is_err());
    assert!(file.optional::<u16>("missing.port").unwrap().is_none());
    assert_eq!(
        file.required::<Vec<String>>("cors.origins").unwrap().len(),
        1
    );
    file.check_keys(&["server.port", "logging.enabled", "cors.origins"])
        .unwrap();
    assert!(file.check_keys(&["server.port"]).is_err());
    assert!(
        ConfigFile::parse("[typo]")
            .unwrap()
            .check_keys(&["server.port"])
            .is_err()
    );
    assert!(
        ConfigFile::parse("[server]\nport = '3000'")
            .unwrap()
            .required::<u16>("server.port")
            .is_err()
    );
    assert!(
        ConfigFile::parse("server = false")
            .unwrap()
            .optional::<String>("server.bind_address")
            .is_err()
    );
}

#[test]
fn toml_syntax_and_conversion_errors_do_not_include_source_values() {
    use xecute::config::ConfigFile;
    let error = ConfigFile::parse("secret = 'DO_NOT_EXPOSE\n")
        .err()
        .unwrap();
    assert!(!error.to_string().contains("DO_NOT_EXPOSE"));
    let file = ConfigFile::parse("[server]\nport = 'DO_NOT_EXPOSE'").unwrap();
    assert!(
        !file
            .required::<u16>("server.port")
            .unwrap_err()
            .to_string()
            .contains("DO_NOT_EXPOSE")
    );
}

#[test]
fn toml_literal_dotted_keys_cannot_silently_bypass_the_schema() {
    use xecute::config::ConfigFile;
    let file = ConfigFile::parse("\"server.port\" = 0").unwrap();
    assert!(file.check_keys(&["server.port"]).is_err());
    let file = ConfigFile::parse("server.port = 3000").unwrap();
    file.check_keys(&["server.port"]).unwrap();
    assert_eq!(file.required::<u16>("server.port").unwrap(), 3000);
    assert!(
        ConfigFile::parse("name = ' '")
            .unwrap()
            .required::<String>("name")
            .is_err()
    );
}
