//! Opt-in JWT bearer authentication and exact-match role/permission policies.
//! Configure a trusted verifier using an Extension layer or DI, then execute
//! authenticate as a guard. Policies consume only verified request principals.

use crate::{HttpError, middleware::Request};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::{fmt, sync::Arc};

/// JWT encoding and claim tooling for trusted issuers and application-specific integrations.
pub use jsonwebtoken as jwt;

/// Static configuration errors; never include keys or token values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthConfigError(&'static str);
impl fmt::Display for AuthConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for AuthConfigError {}

/// Safe authentication or authorization rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthError {
    /// Missing, malformed, expired, or untrusted bearer credentials (401).
    Unauthorized,
    /// Verified user does not satisfy a required policy (403).
    Forbidden,
    /// The application did not install its trusted verifier (500).
    MissingVerifier,
}
impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "Authentication required",
            Self::Forbidden => "Access denied",
            Self::MissingVerifier => "Authentication configuration unavailable",
        })
    }
}
impl std::error::Error for AuthError {}
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::MissingVerifier => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut response = HttpError::new(status).into_response();
        if self == Self::Unauthorized {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                header::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}

/// Identity constructed only after signature and registered claim validation.
/// Roles and permissions are trusted claims from the configured issuer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Principal {
    subject: String,
    roles: Vec<String>,
    permissions: Vec<String>,
    expires_at: u64,
}
impl Principal {
    /// Stable issuer-assigned user or service identifier.
    pub fn subject(&self) -> &str {
        &self.subject
    }
    /// Exact role names; there is no implicit admin bypass.
    pub fn roles(&self) -> &[String] {
        &self.roles
    }
    /// Exact permission strings; no wildcard expansion is performed.
    pub fn permissions(&self) -> &[String] {
        &self.permissions
    }
    /// Expiration as Unix seconds.
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
    /// Check one exact, case-sensitive role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|value| value == role)
    }
    /// Check one exact, case-sensitive permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|value| value == permission)
    }
}

/// Extract the principal installed by an authentication guard; never parses unsigned input.
pub struct AuthenticatedUser(pub Principal);
impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedUser {
    type Rejection = AuthError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .map(Self)
            .ok_or(AuthError::Unauthorized)
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    permissions: Vec<String>,
}

/// A pinned key, algorithm, issuer and audience. Debug output excludes key material.
/// Supports HS256 shared secrets and RS256 public PEM keys; no remote key fetching.
#[derive(Clone)]
pub struct JwtVerifier {
    key: DecodingKey,
    validation: Validation,
}
impl fmt::Debug for JwtVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwtVerifier")
            .field("key", &"[REDACTED]")
            .field("algorithm", &self.validation.algorithms)
            .finish()
    }
}
impl JwtVerifier {
    /// Pin HS256 with a secret of at least 32 bytes. Use cryptographically random key material.
    pub fn hs256(secret: &[u8], issuer: &str, audience: &str) -> Result<Self, AuthConfigError> {
        if secret.len() < 32 {
            return Err(AuthConfigError(
                "HS256 requires at least 32 bytes of key material",
            ));
        }
        Self::new(
            DecodingKey::from_secret(secret),
            Algorithm::HS256,
            issuer,
            audience,
        )
    }
    /// Pin RS256 to a trusted RSA public key in PEM format.
    pub fn rs256(public_key: &[u8], issuer: &str, audience: &str) -> Result<Self, AuthConfigError> {
        let key = DecodingKey::from_rsa_pem(public_key)
            .map_err(|_| AuthConfigError("invalid RSA public key"))?;
        Self::new(key, Algorithm::RS256, issuer, audience)
    }
    fn new(
        key: DecodingKey,
        algorithm: Algorithm,
        issuer: &str,
        audience: &str,
    ) -> Result<Self, AuthConfigError> {
        if issuer.trim().is_empty() || audience.trim().is_empty() {
            return Err(AuthConfigError("issuer and audience are required"));
        }
        let mut validation = Validation::new(algorithm);
        validation.leeway = 0;
        validation.validate_nbf = true;
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        Ok(Self { key, validation })
    }
    /// Verify a JWT without accepting an algorithm or key supplied by the token.
    /// Requires exp, iss, aud, and a nonblank sub; nbf is validated when present.
    pub fn verify(&self, token: &str) -> Result<Principal, AuthError> {
        if token.is_empty() || token.len() > 16_384 {
            return Err(AuthError::Unauthorized);
        }
        let header = jwt::decode_header(token).map_err(|_| AuthError::Unauthorized)?;
        // No critical JOSE extensions are implemented by this verifier.
        if header.crit.is_some() {
            return Err(AuthError::Unauthorized);
        }
        let claims = jwt::decode::<Claims>(token, &self.key, &self.validation)
            .map_err(|_| AuthError::Unauthorized)?
            .claims;
        if claims.sub.trim().is_empty() || claims.exp <= jwt::get_current_timestamp() {
            return Err(AuthError::Unauthorized);
        }
        Ok(Principal {
            subject: claims.sub,
            roles: claims.roles,
            permissions: claims.permissions,
            expires_at: claims.exp,
        })
    }
    /// Authenticate a request using this verifier; useful with a captured Arc or DI.
    /// Always replaces any previously installed principal instead of trusting stale identity.
    pub async fn authenticate(&self, mut request: Request) -> Result<Request, AuthError> {
        request.extensions_mut().remove::<Principal>();
        let principal = self.verify(bearer_token(&request)?)?;
        request.extensions_mut().insert(principal);
        Ok(request)
    }
}

fn bearer_token(request: &Request) -> Result<&str, AuthError> {
    let mut values = request.headers().get_all(header::AUTHORIZATION).iter();
    let value = values.next().ok_or(AuthError::Unauthorized)?;
    if values.next().is_some() {
        return Err(AuthError::Unauthorized);
    }
    let value = value.to_str().map_err(|_| AuthError::Unauthorized)?;
    let (scheme, token) = value.split_once(' ').ok_or(AuthError::Unauthorized)?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(AuthError::Unauthorized);
    }
    Ok(token)
}

/// Guard using an Arc<JwtVerifier> installed by an Axum Extension layer.
/// A missing verifier is an application error, never an anonymous-user fallback.
pub async fn authenticate(request: Request) -> Result<Request, AuthError> {
    let verifier = request
        .extensions()
        .get::<Arc<JwtVerifier>>()
        .cloned()
        .ok_or(AuthError::MissingVerifier)?;
    verifier.authenticate(request).await
}

/// Authenticate using a JwtVerifier provider in the active RequestScope.
/// Use scoped_router, or install with_container outside global guards first.
pub async fn authenticate_injected(request: Request) -> Result<Request, AuthError> {
    let verifier = request
        .extensions()
        .get::<crate::RequestScope>()
        .ok_or(AuthError::MissingVerifier)?
        .resolve::<JwtVerifier>()
        .map_err(|_| AuthError::MissingVerifier)?;
    verifier.authenticate(request).await
}

/// Guard requiring ALL listed roles from an already verified principal.
/// Empty policies are rejected, not treated as public access.
pub async fn require_roles(request: Request, required: &[&str]) -> Result<Request, AuthError> {
    let principal = request
        .extensions()
        .get::<Principal>()
        .ok_or(AuthError::Unauthorized)?;
    if required.is_empty()
        || !required
            .iter()
            .all(|role| !role.trim().is_empty() && principal.has_role(role))
    {
        return Err(AuthError::Forbidden);
    }
    Ok(request)
}

/// Guard requiring ALL listed permissions. No role implicitly grants a permission.
pub async fn require_permissions(
    request: Request,
    required: &[&str],
) -> Result<Request, AuthError> {
    let principal = request
        .extensions()
        .get::<Principal>()
        .ok_or(AuthError::Unauthorized)?;
    if required.is_empty()
        || !required
            .iter()
            .all(|permission| !permission.trim().is_empty() && principal.has_permission(permission))
    {
        return Err(AuthError::Forbidden);
    }
    Ok(request)
}
