# Guards, JWT authentication, and authorization

Xecute supports executable guards at application, module, controller, and HTTP-method
scope. Guards run before request DTO extraction and handler execution. Optional
JWT authentication installs a verified principal; role and permission policies
then authorize that principal.

Enable the runtime's `auth` feature in your application:

```toml
[dependencies]
xecute = { version = "0.1", features = ["auth"] }
```

Custom guards require no feature flag. JWT verification, `AuthenticatedUser`,
`#[roles]`, and `#[permissions]` require `auth`. Existing MVC/CQRS resources remain
public until you attach guards. Generators do not create a login or user table.

## Choose the scope: module, controller, or route

Use `#[guard(...)]` with async guard functions. A guard receives a request and
returns `Result<Request, E>`, where `E` implements `IntoResponse`. Returning an
error stops the request before DTO validation and handler execution.

This custom guard checks a header; JWT verification is shown later in this guide:

```rust
use xecute::{HttpError, HttpStatus, middleware::Request};

async fn require_api_key(request: Request) -> Result<Request, HttpError> {
    // Example policy only: load real credentials from validated configuration.
    if request.headers().get("x-api-key").is_none() {
        return Err(HttpError::new(HttpStatus::UNAUTHORIZED));
    }
    Ok(request)
}
```

The example only checks presence. Do not use header presence as credential
verification; verify the value against your configured policy or use the JWT guard.

### Guard every controller in a module

In a generated MVC user module, add the annotation **below** `#[module(...)]`:

```rust
# use xecute::{module, guard};
#[module(
    providers = [UserRepository, UserService, UserController],
    controllers = [UserController],
    exports = [UserService],
)]
#[guard(require_api_key)]
pub struct UserModule;
```

This is a module fragment: keep the generated imports for your providers and
import the guard function from its owning module. An equivalent declaration is
`#[module(..., guards = [require_api_key])]`; use either style. Multiple guards
run in list order. If both forms are used, `guards = [...]` runs first, followed
by the `#[guard]`, `#[roles]`, and `#[permissions]` annotations in source order.

Newly generated resources mount through `UserModule::scoped_router(...)`, so the
module policy covers every controller in its `controllers` list. Imported modules
are mounted separately; an import controls DI visibility, not HTTP ownership.

For manual routing, merge the **module** router instead of individual controllers:

```rust
let router = axum::Router::new()
    .merge(UserModule::scoped_router(container.clone()));
```

The `container` is an `Arc<Container>` built from all required module definitions.
Request-scoped DI context is installed before module guards, so a guard can resolve
providers through `RequestScope`. Controller types must still be registered as
providers; the `controllers` list does not replace provider registration.

### Guard one controller

```rust
# use xecute::prelude::*;
#[controller("/user")]
#[guard(require_api_key)]
#[derive(Injectable)]
pub struct UserController {}
```

This fragment protects every route in the controller. Use it when other
controllers in the same module should have different policies.

### Guard one route

Inside the controller’s `#[routes]` implementation:

```rust
#[get("/private")]
#[guard(require_api_key)]
async fn private(&self) -> &'static str {
    "Allowed"
}
```

A route guard protects that HTTP method only. Other methods on the same path
keep their own policies. Module/controller guards still apply to every matched
path in their scope, including method-not-allowed requests. Unmatched paths and
separately merged public modules do not inherit another module’s policy.

### Combine scopes and JWT policies

```text
Application middleware → Application guards → Module guards
→ Controller middleware → Controller guards
→ Route middleware → Route guards → DTO extraction → Handler
```

Policies are additive. A route cannot bypass a guard on its controller or module.
Keep login/public routes in a separately mounted public module when necessary.
Module `#[roles(...)]` and `#[permissions(...)]` are supported with the `auth`
feature, just like controller and route policies. Authentication must run before
those checks, for example `#[guard(xecute::auth::authenticate_injected)]` followed
by `#[roles("admin")]` on the module. Configure the verifier provider first.

To guard every generated application route, place `#[guard(...)]` beneath the
root `AppModule`’s `#[module(...)]`. Its generated router applies the root guards
after assembling all resource modules, with DI context outside the guard layer.

### Existing applications

Updating the CLI does not rewrite previously generated routing code. Replace
`UserController::scoped_router(container.clone())` merges with
`UserModule::scoped_router(container.clone())` for each feature. In the root
`router_with_container` helper, after all feature merges, return:

```rust
Ok(xecute::with_container(AppModule::apply_guards(router), container))
```

Apply each module wrapper once, before merging unrelated routes. Calling a
controller router directly does not discover or apply its module’s guards.

## Run the example

From the repository root, generate a development secret and start the server:

```sh
export JWT_SECRET="$(openssl rand -hex 32)"
cargo run -p xecute --example auth --features auth
```

In another terminal, export the **same** development secret, then:

```sh
# The token subcommand is a local demo issuer, not a login endpoint.
ADMIN_TOKEN=$(cargo run --quiet -p xecute --example auth --features auth -- token admin)
READER_TOKEN=$(cargo run --quiet -p xecute --example auth --features auth -- token reader)

curl -i http://127.0.0.1:3000/health
# 200: public route
curl -i http://127.0.0.1:3000/users/me
# 401 + WWW-Authenticate: Bearer
curl -i http://127.0.0.1:3000/users/me -H "Authorization: Bearer $READER_TOKEN"
# 200: verified subject and roles
curl -i http://127.0.0.1:3000/users/admin -H "Authorization: Bearer $READER_TOKEN"
# 403: authenticated, missing admin role
curl -i http://127.0.0.1:3000/users/admin -H "Authorization: Bearer $ADMIN_TOKEN"
# 200: admin role and users:read permission
```

Tokens issued by the example expire after ten minutes. Never use the demo issuer
as production authentication: real issuance must authenticate credentials first.
The server binds only to localhost; use HTTPS at your production ingress.

## Configure a trusted verifier

```rust
use std::sync::Arc;
use axum::Extension;
use xecute::auth::JwtVerifier;

let secret = std::env::var("JWT_SECRET")?;
let verifier = Arc::new(JwtVerifier::hs256(
    secret.as_bytes(), "https://issuer.example", "users-api",
)?);
// Apply to the final router so authentication guards can read the verifier:
let router = router.layer(Extension(verifier));
```

HS256 requires at least 32 bytes of secret material; length alone does not imply
entropy, so generate it cryptographically and store it in a secret manager.
Shared-secret verifiers also possess signing capability. For externally issued
credentials, pin a trusted RS256 public key instead:

```rust
let verifier = JwtVerifier::rs256(
    &std::fs::read("./keys/access-token-public.pem")?,
    "https://issuer.example", "users-api",
)?;
```

Use an appropriately sized RSA key (at least 2048 bits). Key material is loaded
from trusted application configuration. The verifier does not fetch URLs or trust
`jku`, `jwk`, `x5u`, or token-supplied key material. `kid` does not select another
key; this API pins one key. JWKS fetching and automatic key rotation are not
included. Rotate configuration explicitly and plan overlapping validity through
your identity provider and application deployment.

Both constructors require nonblank issuer and audience. Verification pins the
algorithm and checks signature, expiration, issuer, and audience. `exp`, `iss`,
`aud`, and a nonblank string `sub` are required. An audience can be a string or a
list containing the configured audience. Optional `nbf` is checked. Clock leeway
is zero; keep issuer and server clocks synchronized. Unknown critical JOSE
extensions and tokens longer than 16 KiB are rejected. `iat`, token revocation,
and OAuth token-type semantics are not additional policies in this wrapper.
Use a dedicated API audience so tokens intended for another application are not
accepted here.

The supported authorization claim shape is:

```json
{
  "sub": "user-42",
  "iss": "https://issuer.example",
  "aud": "users-api",
  "exp": 2000000000,
  "roles": ["member", "admin"],
  "permissions": ["users:read", "users:write"]
}
```

`roles` and `permissions` are optional arrays of strings and default to empty.
Malformed claim types are rejected. Custom identity-provider claims, nested roles,
and OAuth's space-delimited `scope` are not automatically mapped. Use trusted
issuer configuration or an application-specific authentication guard to adapt
those formats; never create authorization data from unsigned request input.

## Protect controllers and routes

```rust
use xecute::prelude::*;
use xecute::auth::{self, AuthenticatedUser};

#[controller("/users")]
#[guard(auth::authenticate)]
#[roles("member")]
#[derive(Injectable)]
pub struct UserController {}

#[routes]
impl UserController {
    #[get("/me")]
    async fn me(&self, user: AuthenticatedUser) -> String {
        user.0.subject().to_owned()
    }

    #[post("")]
    #[roles("admin")]
    #[permissions("users:write")]
    async fn create(&self) -> &'static str {
        "Authorized"
    }
}
```

Place controller annotations **below** `#[controller]` on the struct. You can
alternatively put controller policies below `#[routes]` on the impl. Route
annotations belong inside the owning `#[routes]` impl on HTTP handlers. Using
these annotations outside their owning macro is a compile error. Legacy
`#[controller("...")] impl ...` syntax is supported as well.

All listed roles and all listed permissions are required, with exact,
case-sensitive matching. There is no implicit admin bypass, wildcard expansion,
or OR policy. Module, controller and route policies are additive: a route cannot weaken
its enclosing scopes' requirements. Empty policies are compile errors (and direct
runtime helper calls deny empty lists). For OR policies or record ownership,
write a custom guard or domain authorization service.

Authentication must execute before role/permission guards. Keep
`#[guard(auth::authenticate)]` above policies in the same scope. `AuthenticatedUser`
only reads an already verified principal; it does not silently parse credentials.
The principal exposes subject, roles, permissions, expiry, `has_role`, and
`has_permission`. Its fields are private and it cannot be deserialized from user
input. Authorization of a specific record still belongs in your application after
loading that record: a users:read permission alone does not establish ownership.
Reuse those checks for HTTP, command handlers, and background jobs.

## Application guards and public routes

```rust
let router = xecute::Application::new(router)
    .use_guard(xecute::auth::authenticate)
    .into_router()
    .layer(axum::Extension(verifier));
```

Application guards protect the entire supplied router, including fallback
responses. To keep health, login, or public endpoints unauthenticated, prefer
guarding only protected controllers or merge a guarded private router with a
separate public router afterward. There is no `#[public]` annotation that bypasses
an already-running application/controller guard. Avoid broad URI-prefix bypasses
inside authentication code.

Controller guards preserve unrelated 404 routes. A method-level guard does not
protect another HTTP method on the same path and does not replace that method's
405 response. Policy changes do not silently add guards to newly generated
controllers: explicitly protect each private controller, or use a global guard.

## Custom guards and dependency injection

A guard is an async function taking the request and returning it on success:

```rust
use xecute::{HttpError, HttpStatus, middleware::Request};

async fn require_feature(request: Request) -> Result<Request, HttpError> {
    let enabled = request.extensions().get::<bool>().copied().unwrap_or(false);
    if !enabled { return Err(HttpError::new(HttpStatus::FORBIDDEN)); }
    Ok(request)
}
```

Attach it with `#[guard(require_feature)]` or
`Application::use_guard(require_feature)`. Multiple guards execute in declaration
order; the first rejection prevents subsequent guards, DTO decoding/validation,
and handler execution. A guard can enrich request extensions but must leave the
body untouched. Return safe error types; custom guard responses are application
code, so Xecute cannot redact arbitrary text you put into them.

For DI-managed authentication, register `JwtVerifier` as a provider value and use
`#[guard(auth::authenticate_injected)]` on a controller built with `scoped_router`.
The guard resolves the verifier through the active `RequestScope`; missing
configuration or failed DI resolves to a safe 500. Export/import the provider's
owning module when sharing it across modules.

Custom guards can resolve services via
`request.extensions().get::<xecute::RequestScope>()` and `scope.resolve::<T>()`.
Module/controller/route guards created by `scoped_router` already have a request scope.
For a global DI guard, install `xecute::with_container(router, container)` **outside**
the assembled Application router, so the scope exists before the guard executes.
With direct `Controller::router`, use the Extension-backed verifier or install
`with_container` explicitly. Existing async lifecycle providers must complete
`initialize_async()` before traffic reaches any guard.

## Execution order

Xecute uses nested router scopes. At each scope, middleware runs before guards:

```text
Application middleware → application guards
  → root module guards → feature module guards
    → controller middleware → controller guards
      → route middleware → route guards
        → request extraction/DTO validation → handler
```

Middleware registration and guard registration are each ordered independently;
calling `use_guard` before `use_middleware` still runs application middleware
first. A controller can attach middleware/policies to both its struct and impl;
then struct scope wraps impl scope, which wraps route scope. This is Xecute's
explicit nesting order; it does not promise NestJS's separate, universal
all-middleware-then-all-guards lifecycle. Install CORS/preflight handling outside
authentication when browsers need unauthenticated OPTIONS handling.

## Errors, OpenAPI, and boundaries

- Missing/malformed/duplicate Authorization headers, invalid tokens, and missing
  principals return 401 with `WWW-Authenticate: Bearer`.
- A valid principal lacking a role/permission returns 403.
- An absent configured verifier returns a safe 500; it never grants access.

Bearer scheme matching is case-insensitive; this implementation requires exactly
one Authorization header and one non-whitespace token. It accepts no query-string
or cookie token fallback. Error bodies use Xecute's safe JSON envelope; wrap your
router with `exceptions::exception_boundary` for request IDs and normalization.
Neither token values, JWT verification reasons, nor key material appear in these
errors. Do not log Authorization headers or bearer tokens in custom middleware.

Guard annotations do not yet add bearer security schemes to generated OpenAPI.
When preparing Swagger documents, add a `components.securitySchemes.bearerAuth`
entry with `type: http`, `scheme: bearer`, and `bearerFormat: JWT`, then add
`security: [{"bearerAuth": []}]` only to protected operations. Document 401/403
responses with existing `#[resp]` annotations. Public operations should remain
explicitly public in the document.

This feature verifies bearer access tokens and applies application policies.
Password registration/login, password hashing, refresh tokens, sessions/cookies,
CSRF protection, logout/revocation, OAuth/OIDC discovery, and tenant/record-level
policies remain separate application/identity-provider responsibilities. Role
changes are reflected when the issuer issues new claims or your application
checks a current policy store; previously issued tokens remain valid until expiry
unless you implement revocation. No authentication storage is auto-generated.

## Tests

```sh
# Core custom guards, without JWT dependencies:
cargo test -p xecute --no-default-features --test guards_http

# Real signature verification, HTTP policy/extractor tests, and runnable demo tests:
cargo test -p xecute --features auth --test auth_http --test guards_http --example auth

# Reject invalid macro policy declarations:
cargo test -p xmacros
```

Tests cover HS256/RS256, wrong keys and algorithms, algorithm confusion,
unsigned/malformed/expired/not-yet-valid tokens, required issuer/audience/subject,
malformed role/permission claims, header ambiguity, safe errors and correlation,
public endpoints, additive policies, body validation ordering, scoped DI, global
and legacy guard execution, and preservation of unrelated methods/paths. PEM
files under `tests/fixtures/auth` are generated **test-only** keys.

Implementation references: [jsonwebtoken verification/validation](https://docs.rs/jsonwebtoken/latest/jsonwebtoken/struct.Validation.html)
and [jsonwebtoken key and crypto backend documentation](https://github.com/Keats/jsonwebtoken).
