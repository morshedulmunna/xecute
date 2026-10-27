//! Publishable procedural macros for local dependency injection and Axum routes.
//!
//! Use routes as `#[get("/health", state = AppState)]`; the explicit state
//! argument makes generated routers reusable outside this workspace.

mod controller;
mod guards;
mod injectable;
mod middleware;
mod module;
mod routes;
mod routes_impl;

use proc_macro::TokenStream;

/// Generate dependency-injection factories for a struct.
#[proc_macro_attribute]
pub fn injectable(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    injectable::expand(item)
}

/// Selects the provider lifecycle used by `#[injectable]` (`singleton`,
/// `transient`, or `request`).
#[proc_macro_attribute]
pub fn scope(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a module as globally visible. It is consumed by `#[module]`.
#[proc_macro_attribute]
pub fn global(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Generate a controller router from method route attributes.
#[proc_macro_attribute]
pub fn controller(attribute: TokenStream, item: TokenStream) -> TokenStream {
    controller::expand(attribute, item)
}

/// Declare module providers, imports, and exported dependencies.
#[proc_macro_attribute]
pub fn module(attribute: TokenStream, item: TokenStream) -> TokenStream {
    module::expand(attribute, item)
}

/// Generate an Axum router for an inherent controller implementation.
#[proc_macro_attribute]
pub fn routes(attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes_impl::expand(attribute, item)
}

/// Declare an HTTP GET handler.
#[proc_macro_attribute]
pub fn get(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Get)
}

/// Declare an HTTP POST handler.
#[proc_macro_attribute]
pub fn post(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Post)
}

/// Declare an HTTP PATCH handler.
#[proc_macro_attribute]
pub fn patch(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Patch)
}

/// Declare an HTTP DELETE handler.
#[proc_macro_attribute]
pub fn delete(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Delete)
}

/// Declare an HTTP PUT handler.
#[proc_macro_attribute]
pub fn put(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Put)
}
/// Declare an HTTP HEAD handler.
#[proc_macro_attribute]
pub fn head(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Head)
}
/// Declare an HTTP OPTIONS handler.
#[proc_macro_attribute]
pub fn options(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    routes::expand(_attribute, item, routes::Method::Options)
}

// Pipeline and extractor markers are deliberately small in this release. They
// preserve the annotated item for `#[routes]`, which owns handler generation.
macro_rules! marker {
    ($($name:ident),* $(,)?) => { $(
        /// Preserve this annotation for the owning route or module macro.
        #[proc_macro_attribute]
        pub fn $name(_attribute: TokenStream, item: TokenStream) -> TokenStream { item }
    )* };
}
marker!(
    interceptor,
    exception,
    validate,
    body,
    query,
    param,
    header,
    state,
    request,
    extension,
    on_module_init,
    on_module_destroy,
    on_bootstrap,
    on_shutdown,
    test_module
);

/// Derive dependency-injection factories without changing the struct declaration.
#[proc_macro_derive(Injectable, attributes(inject, scope))]
pub fn derive_injectable(item: TokenStream) -> TokenStream {
    injectable::derive(item)
}

/// Declare ordered controller or route middleware under `#[controller]` or `#[routes]`.
#[proc_macro_attribute]
pub fn middleware(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    let error = syn::Error::new(proc_macro2::Span::call_site(),
        "place #[middleware(...)] below #[controller(...)] on a struct, or below #[routes] on an impl; route middleware belongs inside a #[routes] impl")
        .into_compile_error();
    let item: proc_macro2::TokenStream = item.into();
    quote::quote!(#error #item).into()
}

// Owning macros consume these attributes. Outside that context, fail explicitly.
macro_rules! guard_annotation {
    ($($name:ident),* $(,)?) => { $(
        /// Declare guards or authorization policy beneath controller/routes, or on a routed handler.
        #[proc_macro_attribute]
        pub fn $name(_attribute: TokenStream, item: TokenStream) -> TokenStream {
            let error = syn::Error::new(proc_macro2::Span::call_site(),
                "place guard/roles/permissions below #[controller] on a struct or #[routes] on an impl; handler policies belong inside #[routes]")
                .into_compile_error();
            let item: proc_macro2::TokenStream = item.into();
            quote::quote!(#error #item).into()
        }
    )* };
}
guard_annotation!(guard, roles, permissions);
