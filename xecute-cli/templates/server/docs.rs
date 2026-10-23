//! Vendored Swagger UI and OpenAPI derived from registered controller metadata.

use axum::{Router, middleware, response::Response};
use utoipa_swagger_ui::{Config as SwaggerConfig, SwaggerUi};
use xecute::{ApiOperation, DocumentationError};

use crate::configs::Config;

/// Trusted marker allowing the documentation assets to use their own CSP.
#[derive(Clone)]
pub(super) struct SwaggerAsset;

async fn mark_documentation(mut response: Response) -> Response {
    response.extensions_mut().insert(SwaggerAsset);
    response
}

/// Build documentation routes before the shared server policies are applied.
pub fn router(
    config: &Config,
    operations: Vec<ApiOperation>,
) -> Result<Router, DocumentationError> {
    if !config.docs.enabled {
        return Ok(Router::new());
    }

    let document = xecute::openapi_document(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        operations,
    )?;

    Ok(Router::new()
        .merge(
            SwaggerUi::new("/docs")
                .external_url_unchecked("/docs/openapi.json", document)
                // Disable remote validation and query-string configuration overrides.
                .config(
                    SwaggerConfig::default()
                        .validator_url("none")
                        .query_config_enabled(false),
                ),
        )
        .layer(middleware::map_response(mark_documentation)))
}

/// Mount independent Swagger UIs and OpenAPI JSON for each enabled API version.
pub fn versioned_router(config: &Config, versions: &xecute::ApiVersions) -> Result<Router, DocumentationError> {
    let mut router = Router::new();
    if !config.docs.enabled { return Ok(router); }
    for (version, document) in versions.documents(env!("CARGO_PKG_NAME"))? {
        router = router.merge(
            SwaggerUi::new(format!("/docs/{version}"))
                .external_url_unchecked(format!("/docs/{version}/openapi.json"), document)
                .config(SwaggerConfig::default().validator_url("none").query_config_enabled(false))
        );
    }
    Ok(router.layer(middleware::map_response(mark_documentation)))
}
