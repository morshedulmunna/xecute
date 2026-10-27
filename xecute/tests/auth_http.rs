//! JWT trust boundaries, authorization policies, and controller integration.
#![cfg(feature = "auth")]
use axum::{
    Extension, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use xecute::{
    auth::{
        self, AuthError, AuthenticatedUser, JwtVerifier,
        jwt::{self, Algorithm, EncodingKey, Header},
    },
    prelude::*,
};

const SECRET: &[u8] = b"test-only-hmac-key-32-bytes-minimum!!";
fn verifier() -> JwtVerifier {
    JwtVerifier::hs256(SECRET, "test-issuer", "test-api").unwrap()
}
fn claims() -> Value {
    json!({"sub":"user-42", "exp":jwt::get_current_timestamp()+3600, "iss":"test-issuer", "aud":"test-api", "roles":["member","admin"], "permissions":["users:read","users:write"]})
}
fn token(claims: &Value) -> String {
    jwt::encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(SECRET),
    )
    .unwrap()
}

#[test]
fn jwt_validates_signature_algorithm_required_claims_and_time() {
    let verifier = verifier();
    let good = token(&claims());
    let user = verifier.verify(&good).unwrap();
    assert_eq!(user.subject(), "user-42");
    assert!(user.has_role("admin"));
    assert!(user.has_permission("users:read"));
    assert!(!user.has_permission("users:*"));
    assert!(!user.has_role("ADMIN"));
    for (key, value) in [
        ("iss", json!("attacker")),
        ("aud", json!("different-api")),
        ("exp", json!(jwt::get_current_timestamp() - 100)),
        ("exp", json!(jwt::get_current_timestamp())),
        ("nbf", json!(jwt::get_current_timestamp() + 3600)),
        ("sub", json!("  ")),
        ("roles", json!("admin")),
        ("permissions", json!("users:read")),
    ] {
        let mut altered = claims();
        altered[key] = value;
        assert_eq!(
            verifier.verify(&token(&altered)),
            Err(AuthError::Unauthorized),
            "{key}"
        );
    }
    for key in ["exp", "iss", "aud", "sub"] {
        let mut altered = claims();
        altered.as_object_mut().unwrap().remove(key);
        assert_eq!(
            verifier.verify(&token(&altered)),
            Err(AuthError::Unauthorized)
        );
    }
    let wrong_key = jwt::encode(
        &Header::default(),
        &claims(),
        &EncodingKey::from_secret(b"another-test-secret-at-least-32-bytes"),
    )
    .unwrap();
    let wrong_alg = jwt::encode(
        &Header::new(Algorithm::HS384),
        &claims(),
        &EncodingKey::from_secret(SECRET),
    )
    .unwrap();
    let critical = Header {
        crit: Some(vec!["custom".into()]),
        ..Header::default()
    };
    let unsupported = jwt::encode(&critical, &claims(), &EncodingKey::from_secret(SECRET)).unwrap();
    for bad in [
        wrong_key,
        wrong_alg,
        unsupported,
        "bad.token".into(),
        format!(
            "{}.{}.",
            good.split('.').next().unwrap(),
            good.split('.').nth(1).unwrap()
        ),
        "x".repeat(16_385),
    ] {
        assert_eq!(verifier.verify(&bad), Err(AuthError::Unauthorized));
    }
    let mut multiple_audiences = claims();
    multiple_audiences["aud"] = json!(["another-api", "test-api"]);
    verifier.verify(&token(&multiple_audiences)).unwrap();
}

#[test]
fn rsa_verifier_accepts_pinned_key_and_rejects_algorithm_confusion() {
    let public = include_bytes!("fixtures/auth/test_public.pem");
    let verifier = JwtVerifier::rs256(public, "test-issuer", "test-api").unwrap();
    let key = EncodingKey::from_rsa_pem(include_bytes!("fixtures/auth/test_private.pem")).unwrap();
    let signed = jwt::encode(&Header::new(Algorithm::RS256), &claims(), &key).unwrap();
    verifier.verify(&signed).unwrap();
    let confusion = jwt::encode(
        &Header::new(Algorithm::HS256),
        &claims(),
        &EncodingKey::from_secret(public),
    )
    .unwrap();
    assert_eq!(verifier.verify(&confusion), Err(AuthError::Unauthorized));
    assert_eq!(
        JwtVerifier::hs256(SECRET, "test-issuer", "test-api")
            .unwrap()
            .verify(&signed),
        Err(AuthError::Unauthorized)
    );
}

#[test]
fn configuration_rejects_weak_or_missing_values_and_redacts_keys() {
    assert!(JwtVerifier::hs256(b"weak", "issuer", "audience").is_err());
    assert!(JwtVerifier::hs256(SECRET, "", "audience").is_err());
    assert!(JwtVerifier::hs256(SECRET, "issuer", " ").is_err());
    let bad_key = JwtVerifier::rs256(b"secret invalid pem", "issuer", "audience").unwrap_err();
    assert!(!format!("{bad_key:?} {bad_key}").contains("secret invalid pem"));
    assert!(!format!("{:?}", verifier()).contains(std::str::from_utf8(SECRET).unwrap()));
}

#[derive(serde::Deserialize, validator::Validate, schemars::JsonSchema)]
struct Payload {
    #[validate(length(min = 3))]
    name: String,
}
#[controller("/users")]
#[guard(auth::authenticate)]
#[roles("member")]
#[derive(Injectable)]
struct UserController {
    #[inject(default)]
    calls: AtomicUsize,
}
#[routes]
impl UserController {
    #[get("/me")]
    async fn me(&self, user: AuthenticatedUser) -> Json<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Json(json!({"subject":user.0.subject()}))
    }
    #[get("/admin")]
    #[roles("admin")]
    #[permissions("users:read", "users:write")]
    async fn admin(&self) -> &'static str {
        self.calls.fetch_add(1, Ordering::SeqCst);
        "ok"
    }
    #[post("")]
    #[permissions("users:write")]
    async fn create(&self, #[body] body: Payload) -> String {
        self.calls.fetch_add(1, Ordering::SeqCst);
        body.name
    }
}

fn app(scoped: bool) -> (Router, Arc<UserController>) {
    let c = Arc::new(
        xecute::Container::builder()
            .injectable::<UserController>()
            .build()
            .unwrap(),
    );
    let controller: Arc<UserController> = c.resolve().unwrap();
    let routes = if scoped {
        UserController::scoped_router(c)
    } else {
        UserController::router(controller.clone())
    };
    let app = routes
        .merge(Router::new().route("/public", axum::routing::get(|| async { "public" })))
        .layer(Extension(Arc::new(verifier())));
    (xecute::exceptions::exception_boundary(app), controller)
}
async fn response(
    app: &Router,
    method: &str,
    uri: &str,
    authorization: Option<&str>,
    body: &'static str,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    app.clone()
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn protected_routes_extract_verified_identity_in_both_router_modes() {
    for scoped in [false, true] {
        let (app, c) = app(scoped);
        let signed = token(&claims());
        let response = response(
            &app,
            "GET",
            "/users/me",
            Some(&format!("bEaReR {signed}")),
            "",
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let json: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(json["subject"], "user-42");
        assert_eq!(c.calls.load(Ordering::SeqCst), 1);
        assert_eq!(response_status(&app, "/public", None).await, 200);
        assert_eq!(response_status(&app, "/missing", None).await, 404);
    }
}
async fn response_status(app: &Router, uri: &str, header: Option<&str>) -> u16 {
    response(app, "GET", uri, header, "")
        .await
        .status()
        .as_u16()
}

#[tokio::test]
async fn unauthorized_errors_have_safe_envelope_correlation_and_bearer_challenge() {
    let (app, c) = app(false);
    for authorization in [
        None,
        Some("Basic secret"),
        Some("Bearer"),
        Some("Bearer "),
        Some("Bearer bad token"),
        Some("Bearer invalid.token.secret"),
    ] {
        let response = response(&app, "POST", "/users", authorization, "malformed JSON").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["error"]["code"], "unauthorized");
        assert!(!json["error"]["request_id"].as_str().unwrap().is_empty());
        assert!(!std::str::from_utf8(&bytes).unwrap().contains("secret"));
    }
    assert_eq!(c.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn all_roles_and_permissions_are_required_before_validation_or_handler() {
    let (app, c) = app(false);
    for (roles, permissions, expected) in [
        (
            json!(["member", "admin"]),
            json!(["users:read", "users:write"]),
            200,
        ),
        (json!(["admin"]), json!(["users:read", "users:write"]), 403),
        (json!(["member"]), json!(["users:read", "users:write"]), 403),
        (json!(["member", "admin"]), json!(["users:read"]), 403),
        (json!(["member", "admin"]), json!(["*"]), 403),
    ] {
        let mut claims = claims();
        claims["roles"] = roles;
        claims["permissions"] = permissions;
        let authorization = format!("Bearer {}", token(&claims));
        let response = response(&app, "GET", "/users/admin", Some(&authorization), "").await;
        assert_eq!(response.status().as_u16(), expected);
        if expected == 403 {
            assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        }
    }
    assert_eq!(c.calls.load(Ordering::SeqCst), 1);
    let mut limited = claims();
    limited["permissions"] = json!([]);
    let authorization = format!("Bearer {}", token(&limited));
    assert_eq!(
        response(&app, "POST", "/users", Some(&authorization), "invalid")
            .await
            .status(),
        403
    );
    let authorization = format!("Bearer {}", token(&claims()));
    assert_eq!(
        response(&app, "POST", "/users", Some(&authorization), "invalid")
            .await
            .status(),
        400
    );
    assert_eq!(
        response(
            &app,
            "POST",
            "/users",
            Some(&authorization),
            r#"{"name":"x"}"#
        )
        .await
        .status(),
        422
    );
    assert_eq!(
        response(
            &app,
            "POST",
            "/users",
            Some(&authorization),
            r#"{"name":"Munna"}"#
        )
        .await
        .status(),
        200
    );
}

#[tokio::test]
async fn duplicate_authorization_headers_are_rejected() {
    let (app, _) = app(false);
    let signed = token(&claims());
    let request = Request::builder()
        .uri("/users/me")
        .header(header::AUTHORIZATION, format!("Bearer {signed}"))
        .header(header::AUTHORIZATION, format!("Bearer {signed}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(request).await.unwrap().status(), 401);
}

#[tokio::test]
async fn missing_verifier_is_safe_500_and_missing_principal_is_401() {
    let c = Arc::new(UserController {
        calls: AtomicUsize::new(0),
    });
    let app = UserController::router(c);
    let signed = token(&claims());
    let authorization = format!("Bearer {signed}");
    let response = response(&app, "GET", "/users/me", Some(&authorization), "").await;
    assert_eq!(response.status(), 500);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(!std::str::from_utf8(&body).unwrap().contains("verifier"));
    let router = Router::new().route(
        "/identity",
        axum::routing::get(|user: AuthenticatedUser| async move { user.0.subject().to_owned() }),
    );
    assert_eq!(
        response_status(&router, "/identity", Some(&authorization)).await,
        401
    );
    let router = Application::new(Router::new().route("/", axum::routing::get(|| async { "ok" })))
        .use_guard(|request| async { auth::require_roles(request, &["admin"]).await })
        .into_router();
    assert_eq!(
        response_status(&router, "/", Some(&authorization)).await,
        401
    );
}

#[tokio::test]
async fn empty_policies_deny_and_reauthentication_never_trusts_old_principal() {
    let verifier = verifier();
    let signed = token(&claims());
    let request = Request::builder()
        .header(header::AUTHORIZATION, format!("Bearer {signed}"))
        .body(Body::empty())
        .unwrap();
    let mut request = verifier.authenticate(request).await.unwrap();
    assert!(matches!(
        auth::require_roles(request, &[]).await,
        Err(AuthError::Forbidden)
    ));
    request = Request::builder()
        .header(header::AUTHORIZATION, "Bearer invalid")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(verifier.verify(&signed).unwrap());
    assert!(matches!(
        verifier.authenticate(request).await,
        Err(AuthError::Unauthorized)
    ));
}

#[tokio::test]
async fn authentication_can_resolve_verifier_from_container_scope() {
    #[controller("/injected")]
    #[guard(auth::authenticate_injected)]
    #[derive(Injectable)]
    struct InjectedController {}
    #[routes]
    impl InjectedController {
        #[get("")]
        #[permissions("users:read")]
        async fn get(&self, user: AuthenticatedUser) -> String {
            user.0.subject().to_owned()
        }
    }
    let c = Arc::new(
        xecute::Container::builder()
            .value(verifier())
            .injectable::<InjectedController>()
            .build()
            .unwrap(),
    );
    let app = InjectedController::scoped_router(c);
    let authorization = format!("Bearer {}", token(&claims()));
    assert_eq!(
        response_status(&app, "/injected", Some(&authorization)).await,
        200
    );
    assert_eq!(response_status(&app, "/injected", None).await, 401);
}

#[tokio::test]
async fn module_authentication_and_additive_policies_protect_declared_controllers() {
    #[controller("/module-auth")]
    #[derive(Injectable)]
    struct ModuleController {}
    #[routes]
    impl ModuleController {
        #[get("")]
        #[permissions("users:write")]
        async fn get(&self, user: AuthenticatedUser) -> String {
            user.0.subject().to_owned()
        }
    }
    #[xecute::module(providers = [ModuleController], controllers = [ModuleController])]
    #[guard(auth::authenticate_injected)]
    #[roles("admin")]
    #[permissions("users:read")]
    struct PrivateModule;
    let container = Arc::new(
        xecute::ModuleGraph::new(vec![
            PrivateModule::definition().provider(xecute::ProviderDefinition::value(verifier())),
        ])
        .build()
        .unwrap(),
    );
    let app = PrivateModule::scoped_router(container);
    assert_eq!(response_status(&app, "/module-auth", None).await, 401);
    for (roles, permissions, status) in [
        (json!(["member"]), json!(["users:read", "users:write"]), 403),
        (json!(["admin"]), json!(["users:write"]), 403),
        (json!(["admin"]), json!(["users:read"]), 403),
        (json!(["admin"]), json!(["users:read", "users:write"]), 200),
    ] {
        let mut payload = claims();
        payload["roles"] = roles;
        payload["permissions"] = permissions;
        let authorization = format!("Bearer {}", token(&payload));
        assert_eq!(
            response_status(&app, "/module-auth", Some(&authorization)).await,
            status
        );
    }
}
