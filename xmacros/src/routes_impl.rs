//! Generate HTTP adapters, controller routing, and declared route documentation.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, ImplItem, ItemImpl, LitInt, LitStr, Pat, Token, Type,
    parse::{Parse, ParseStream},
};

struct ResponseArgs {
    status: LitInt,
    description: LitStr,
    json: Option<Type>,
}

impl Parse for ResponseArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let status = input.parse()?;
        input.parse::<Token![,]>()?;
        let description = input.parse()?;
        let json = if input.is_empty() {
            None
        } else {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                None
            } else {
                let key: syn::Ident = input.parse()?;
                if key != "json" {
                    return Err(syn::Error::new_spanned(key, "expected json = ResponseDto"));
                }
                input.parse::<Token![=]>()?;
                Some(input.parse()?)
            }
        };
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            status,
            description,
            json,
        })
    }
}

struct BodyArgs(Type);

impl Parse for BodyArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: syn::Ident = input.parse()?;
        if key != "json" {
            return Err(syn::Error::new_spanned(key, "expected json = RequestDto"));
        }
        input.parse::<Token![=]>()?;
        let ty = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        Ok(Self(ty))
    }
}

/// Generate adapters without exposing extractor or validation plumbing in handlers.
pub fn expand(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match generate(attribute.into(), item.into()) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn generate(
    attribute: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let uses_struct_prefix = attribute.is_empty();
    let prefix = if uses_struct_prefix {
        quote!(Self::__XECUTE_CONTROLLER_PREFIX)
    } else {
        let prefix = syn::parse2::<LitStr>(attribute)?;
        if !prefix.value().is_empty() && !prefix.value().starts_with('/') {
            return Err(syn::Error::new_spanned(
                prefix,
                "controller prefix must start with /",
            ));
        }
        quote!(#prefix)
    };
    let mut implementation: ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some() || !implementation.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &implementation,
            "routing requires a non-generic inherent impl",
        ));
    }
    let controller_middleware = crate::middleware::take(&mut implementation.attrs)?;
    let mut routes = Vec::new();
    let mut scoped_routes = Vec::new();
    let mut adapters = Vec::new();
    let mut operations = Vec::new();

    for item in &mut implementation.items {
        let ImplItem::Fn(method) = item else { continue };
        if matches!(
            method.sig.ident.to_string().as_str(),
            "router" | "scoped_router" | "api_operations"
        ) {
            return Err(syn::Error::new_spanned(
                &method.sig,
                "method name is reserved by #[routes]",
            ));
        }
        let route_middleware = crate::middleware::take(&mut method.attrs)?;
        let mut route = None;
        let mut retained = Vec::new();
        let mut summary = quote!("");
        let mut tag = quote!("");
        let handler = &method.sig.ident;
        let owner = &implementation.self_ty;
        let default_id = format!(
            "{}.{}",
            quote!(#owner).to_string().replace(' ', ""),
            handler
        );
        let mut operation_id = quote!(#default_id);
        let mut body_type = quote!(None);
        let mut body_schema = quote!(None);
        let mut query_schema = quote!(None);
        let mut declared_body = None;
        let mut responses = Vec::new();

        for attribute in std::mem::take(&mut method.attrs) {
            let segment = attribute.path().segments.last().expect("attribute path");
            let name = segment.ident.to_string();
            match name.as_str() {
                "get" | "post" | "put" | "patch" | "delete" | "head" | "options" => {
                    if route.is_some() {
                        return Err(syn::Error::new_spanned(
                            attribute,
                            "one HTTP method annotation is allowed per handler",
                        ));
                    }
                    let path = attribute.parse_args::<LitStr>()?;
                    if !path.value().is_empty() && !path.value().starts_with('/') {
                        return Err(syn::Error::new_spanned(
                            path,
                            "route path must start with /",
                        ));
                    }
                    route = Some((segment.ident.clone(), path));
                }
                "api" => {
                    attribute.parse_nested_meta(|meta| {
                        let value: LitStr = meta.value()?.parse()?;
                        if meta.path.is_ident("summary") {
                            summary = quote!(#value);
                        } else if meta.path.is_ident("tag") {
                            tag = quote!(#value);
                        } else if meta.path.is_ident("operation_id") {
                            operation_id = quote!(#value);
                        } else {
                            return Err(meta.error("expected summary, tag, or operation_id"));
                        }
                        Ok(())
                    })?;
                }
                "body" => {
                    let BodyArgs(ty) = attribute.parse_args()?;
                    body_type = quote!(Some(stringify!(#ty)));
                    body_schema = quote!(Some(::xecute::json_schema::<#ty>()));
                    declared_body = Some(ty);
                }
                "resp" => {
                    let ResponseArgs {
                        status,
                        description,
                        json,
                    } = attribute.parse_args()?;
                    let code = status.base10_parse::<u16>()?;
                    if !(100..=599).contains(&code) {
                        return Err(syn::Error::new_spanned(
                            status,
                            "HTTP status must be between 100 and 599",
                        ));
                    }
                    let json_type = json
                        .as_ref()
                        .map_or(quote!(None), |ty| quote!(Some(stringify!(#ty))));
                    let schema = json.as_ref().map_or(
                        quote!(None),
                        |ty| quote!(Some(::xecute::json_schema::<#ty>())),
                    );
                    responses.push(quote!(::xecute::ApiResponse { status: #code, description: #description, json_type: #json_type, schema: #schema }));
                }
                _ => retained.push(attribute),
            }
        }
        method.attrs = retained;
        let Some((verb, suffix)) = route else {
            if !route_middleware.is_empty() {
                return Err(syn::Error::new_spanned(
                    &method.sig,
                    "middleware requires an HTTP route attribute",
                ));
            }
            continue;
        };
        let receiver = method.sig.receiver().is_some();
        if let Some(receiver) = method.sig.receiver()
            && (!matches!(&receiver.kind, syn::ReceiverKind::Reference(_, _, None))
                || receiver.mutability.is_some())
        {
            return Err(syn::Error::new_spanned(
                receiver,
                "controller handlers require &self",
            ));
        }
        let mut inputs = Vec::new();
        let mut calls = Vec::new();
        let mut body_input = None;
        let mut actual_body = None;
        let mut has_markers = false;
        for argument in &mut method.sig.inputs {
            let FnArg::Typed(argument) = argument else {
                continue;
            };
            let mut marker = None;
            let mut attrs = Vec::new();
            for attr in std::mem::take(&mut argument.attrs) {
                let name = attr.path().segments.last().unwrap().ident.to_string();
                if matches!(name.as_str(), "body" | "query" | "param") {
                    if marker.is_some() {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "only one extractor annotation is allowed",
                        ));
                    }
                    marker = Some(name);
                } else {
                    attrs.push(attr);
                }
            }
            argument.attrs = attrs;
            let Pat::Ident(pattern) = argument.pat.as_ref() else {
                if receiver {
                    return Err(syn::Error::new_spanned(
                        argument,
                        "controller parameters must have simple names",
                    ));
                }
                continue;
            };
            let name = &pattern.ident;
            let ty = &argument.ty;
            calls.push(quote!(#name));
            match marker.as_deref() {
                Some("body") => {
                    has_markers = true;
                    if body_input.is_some() {
                        return Err(syn::Error::new_spanned(
                            argument,
                            "only one JSON body parameter is allowed",
                        ));
                    }
                    actual_body = Some(ty.as_ref().clone());
                    body_input =
                        Some(quote!(::xecute::ValidatedJson(#name): ::xecute::ValidatedJson<#ty>));
                }
                Some("query") => {
                    has_markers = true;
                    query_schema = quote!(Some(::xecute::json_schema::<#ty>()));
                    inputs.push(
                        quote!(::xecute::ValidatedQuery(#name): ::xecute::ValidatedQuery<#ty>),
                    );
                }
                Some("param") => {
                    has_markers = true;
                    inputs.push(quote!(::axum::extract::Path(#name): ::axum::extract::Path<#ty>));
                }
                _ => inputs.push(quote!(#name: #ty)),
            }
        }
        if let Some(declared) = declared_body {
            if actual_body.as_ref().map(|ty| quote!(#ty).to_string())
                != Some(quote!(#declared).to_string())
            {
                return Err(syn::Error::new_spanned(
                    declared,
                    "body metadata must match the #[body] parameter type",
                ));
            }
        } else if let Some(ty) = actual_body {
            body_type = quote!(Some(stringify!(#ty)));
            body_schema = quote!(Some(::xecute::json_schema::<#ty>()));
        }
        let mut scoped_handler = handler.clone();
        let selected_handler = if receiver || has_markers {
            if method.sig.asyncness.is_none() || !method.sig.generics.params.is_empty() {
                return Err(syn::Error::new_spanned(
                    &method.sig,
                    "controller handlers must be async and non-generic",
                ));
            }
            if let Some(body) = body_input {
                inputs.push(body);
            }
            let adapter = format_ident!("__xecute_handler_{}", handler);
            let output = &method.sig.output;
            let invocation = if receiver {
                quote!(controller.#handler(#(#calls),*).await)
            } else {
                quote!(Self::#handler(#(#calls),*).await)
            };
            scoped_handler = format_ident!("__xecute_scoped_handler_{}", handler);
            adapters.push(syn::parse2::<ImplItem>(quote! {
                async fn #scoped_handler(
                    ::xecute::Inject(controller): ::xecute::Inject<Self>,
                    #(#inputs),*
                ) #output { #invocation }
            })?);
            adapters.push(syn::parse2::<ImplItem>(quote! {
                async fn #adapter(
                    ::axum::extract::State(controller): ::axum::extract::State<::std::sync::Arc<Self>>,
                    #(#inputs),*
                ) #output { #invocation }
            })?);
            adapter
        } else {
            handler.clone()
        };
        let suffix = suffix.value();
        let path = quote!({
            let path = format!("{}{}", (#prefix).trim_end_matches('/'), #suffix);
            if path.is_empty() {
                "/".to_owned()
            } else {
                path
            }
        });
        let mut scoped_method_router = quote!(::axum::routing::#verb(Self::#scoped_handler));
        let mut method_router = quote!(::axum::routing::#verb(Self::#selected_handler));
        for function in route_middleware.iter().rev() {
            method_router = quote!(#method_router.layer(::axum::middleware::from_fn(#function)));
            scoped_method_router =
                quote!(#scoped_method_router.layer(::axum::middleware::from_fn(#function)));
        }
        routes.push(quote!(.route(&#path, #method_router)));
        scoped_routes.push(quote!(.route(&#path, #scoped_method_router)));
        let method_name = verb.to_string().to_uppercase();
        operations.push(quote!(::xecute::ApiOperation { method: #method_name, path: #path, summary: #summary, tag: #tag, operation_id: #operation_id, body_type: #body_type, body_schema: #body_schema, query_schema: #query_schema, responses: vec![#(#responses),*] }));
    }
    if routes.is_empty() {
        return Err(syn::Error::new_spanned(
            &implementation,
            "routing requires at least one HTTP method attribute",
        ));
    }
    implementation.items.extend(adapters);
    let router = quote!(::axum::Router::new() #(#routes)* .with_state(controller));
    let router = crate::middleware::wrap(router, &controller_middleware);
    let router = if uses_struct_prefix {
        quote!(Self::__xecute_controller_middleware(#router))
    } else {
        router
    };
    implementation.items.push(syn::parse_quote! {
        /// Build this controller’s router and inject its shared state.
        pub fn router(controller: ::std::sync::Arc<Self>) -> ::axum::Router {
            #router
        }
    });
    let scoped_router = quote!(::axum::Router::new() #(#scoped_routes)*);
    let scoped_router = crate::middleware::wrap(scoped_router, &controller_middleware);
    let scoped_router = if uses_struct_prefix {
        quote!(Self::__xecute_controller_middleware(#scoped_router))
    } else {
        scoped_router
    };
    implementation.items.push(syn::parse_quote! {
        /// Resolve the controller for each request, respecting all dependency scopes.
        /// Register the controller as a provider before calling this method.
        pub fn scoped_router(container: ::std::sync::Arc<::xecute::Container>) -> ::axum::Router {
            ::xecute::with_container(#scoped_router, container)
        }
    });
    implementation.items.push(syn::parse_quote! {
        /// Return declared route documentation and response metadata.
        pub fn api_operations() -> Vec<::xecute::ApiOperation> { vec![#(#operations),*] }
    });
    Ok(quote!(#implementation))
}

#[cfg(test)]
mod tests {
    use super::generate;
    use quote::quote;

    #[test]
    fn rejects_empty_middleware_and_middleware_on_non_routes() {
        for item in [
            quote! { impl Controller { #[get("")] #[middleware()] async fn get() {} } },
            quote! { impl Controller { #[middleware(auth)] async fn helper() {} #[get("")] async fn get() {} } },
            quote! { #[middleware()] impl Controller { #[get("")] async fn get() {} } },
        ] {
            assert!(generate(quote!("/test"), item).is_err());
        }
    }
}
