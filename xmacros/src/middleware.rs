//! Shared middleware annotation parsing and ordered router wrapping.

use quote::quote;
use syn::{Attribute, Path, Token, punctuated::Punctuated};

pub fn take(attributes: &mut Vec<Attribute>) -> syn::Result<Vec<Path>> {
    let mut middleware = Vec::new();
    let mut retained = Vec::new();
    for attribute in std::mem::take(attributes) {
        if attribute
            .path()
            .segments
            .last()
            .is_some_and(|s| s.ident == "middleware")
        {
            let paths =
                attribute.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
            if paths.is_empty() {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "expected at least one middleware function",
                ));
            }
            middleware.extend(paths);
        } else {
            retained.push(attribute);
        }
    }
    *attributes = retained;
    Ok(middleware)
}

pub fn wrap(mut router: proc_macro2::TokenStream, middleware: &[Path]) -> proc_macro2::TokenStream {
    for function in middleware.iter().rev() {
        router = quote!(::xecute::middleware::apply_routes(#router, #function));
    }
    router
}
