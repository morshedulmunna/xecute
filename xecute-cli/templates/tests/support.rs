//! Shared test fixtures with explicit socket peers to exercise the real limiter.

use crate::{configs::Config, server::router};
use axum::{
    Json, Router, body::Body, extract::ConnectInfo, http::Request as HttpRequest, routing::get,
};
use std::net::SocketAddr;

pub(super) fn request(path: &str, ip: &str) -> HttpRequest<Body> {
    let mut request = HttpRequest::builder()
        .uri(path)
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        format!("{ip}:4000").parse::<SocketAddr>().unwrap(),
    ));
    request
}

pub(super) fn app(config: &Config) -> Router {
    router(
        Router::new().route("/items", get(|| async { Json(Vec::<String>::new()) })),
        config,
    )
}
