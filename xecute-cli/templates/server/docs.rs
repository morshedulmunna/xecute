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
