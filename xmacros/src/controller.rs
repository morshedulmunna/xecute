//! Controller prefixes shared by the struct annotation and routing implementation.
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, LitStr};

/// Declare a struct prefix, retaining support for the original impl annotation.
pub fn expand(attribute: TokenStream, item: TokenStream) -> TokenStream {
    if let Ok(mut structure) = syn::parse::<ItemStruct>(item.clone()) {
        let prefix = match syn::parse::<LitStr>(attribute) {
            Ok(prefix) if prefix.value().is_empty() || prefix.value().starts_with('/') => prefix,
            Ok(prefix) => {
                return syn::Error::new_spanned(prefix, "controller prefix must start with /")
                    .into_compile_error()
                    .into();
            }
            Err(error) => return error.into_compile_error().into(),
        };
        if !structure.generics.params.is_empty() {
            return syn::Error::new_spanned(&structure, "controller structs must not be generic")
                .into_compile_error()
                .into();
        }
        let middleware = match crate::middleware::take(&mut structure.attrs) {
            Ok(middleware) => middleware,
            Err(error) => return error.into_compile_error().into(),
        };
        let wrapped = crate::middleware::wrap(quote!(router), &middleware);
        let name = &structure.ident;
        return quote! {
            #structure
            impl #name {
                const __XECUTE_CONTROLLER_PREFIX: &'static str = #prefix;
                fn __xecute_controller_middleware(router: ::axum::Router) -> ::axum::Router {
                    #wrapped
                }
            }
        }
        .into();
    }
    crate::routes_impl::expand(attribute, item)
}
