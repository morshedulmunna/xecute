//! JWT_SECRET=<random key of at least 32 bytes> cargo run -p xecute --example auth --features auth
//! Issue local demo credentials with the same command followed by: -- token admin|reader
use axum::{Extension, Router};
use std::sync::Arc;
use xecute::{
    auth::{self, AuthenticatedUser, JwtVerifier, jwt},
    prelude::*,
};

#[controller("/users")]
#[guard(auth::authenticate)]
#[derive(Injectable)]
struct UserController {}

#[routes]
impl UserController {
    #[get("/me")]
    async fn me(&self, user: AuthenticatedUser) -> Json<serde_json::Value> {
        Json(serde_json::json!({"subject": user.0.subject(), "roles": user.0.roles()}))
    }

    #[get("/admin")]
    #[roles("admin")]
    #[permissions("users:read")]
    async fn admin(&self) -> &'static str {
        "Admin access granted"
    }
}

fn router(verifier: JwtVerifier) -> Router {
    let public = Router::new().route("/health", axum::routing::get(|| async { "ok" }));
    let protected = UserController::router(Arc::new(UserController {}));
    xecute::exceptions::exception_boundary(
        public.merge(protected).layer(Extension(Arc::new(verifier))),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let secret = std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET is required")?;
    let verifier = JwtVerifier::hs256(secret.as_bytes(), "xecute-demo", "xecute-api")?;
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if !arguments.is_empty() {
        if arguments.len() != 2
            || arguments[0] != "token"
            || !matches!(arguments[1].as_str(), "admin" | "reader")
        {
            return Err("usage: auth [token admin|reader]".into());
        }
        // Local demo issuer only. Production issuance must first authenticate credentials.
        let role = &arguments[1];
        let claims = serde_json::json!({
            "sub": "demo-user", "iss": "xecute-demo", "aud": "xecute-api",
            "exp": jwt::get_current_timestamp() + 600,
            "roles": [role], "permissions": ["users:read"],
        });
        let token = jwt::encode(
            &jwt::Header::default(),
            &claims,
            &jwt::EncodingKey::from_secret(secret.as_bytes()),
        )?;
        println!("{token}");
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!(
        "Auth example: http://127.0.0.1:3000; public /health, protected /users/me and /users/admin"
    );
    axum::serve(listener, router(verifier)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    #[tokio::test]
    async fn demo_keeps_health_public_and_enforces_admin_policy() {
        let secret = b"demo-test-secret-at-least-32-bytes!!";
        let app = router(JwtVerifier::hs256(secret, "xecute-demo", "xecute-api").unwrap());
        let claims = serde_json::json!({"sub":"reader", "iss":"xecute-demo", "aud":"xecute-api", "exp":jwt::get_current_timestamp()+600, "roles":["reader"], "permissions":["users:read"]});
        let token = jwt::encode(
            &jwt::Header::default(),
            &claims,
            &jwt::EncodingKey::from_secret(secret),
        )
        .unwrap();
        for (path, authorization, status) in [
            ("/health", None, 200),
            ("/users/me", None, 401),
            ("/users/me", Some(format!("Bearer {token}")), 200),
            ("/users/admin", Some(format!("Bearer {token}")), 403),
        ] {
            let mut request = Request::builder().uri(path);
            if let Some(value) = authorization {
                request = request.header("authorization", value);
            }
            assert_eq!(
                app.clone()
                    .oneshot(request.body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status()
                    .as_u16(),
                status
            );
        }
    }
}
