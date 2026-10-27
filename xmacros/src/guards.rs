//! Parse executable guard and authorization annotations for controller macros.
use quote::quote;
use syn::{Attribute, LitStr, Path, Token, punctuated::Punctuated};

pub fn take(attributes: &mut Vec<Attribute>) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut guards = Vec::new();
    let mut retained = Vec::new();
    for attribute in std::mem::take(attributes) {
        let name = attribute.path().segments.last().unwrap().ident.to_string();
        match name.as_str() {
            "guard" => {
                let paths =
                    attribute.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
                if paths.is_empty() {
                    return Err(syn::Error::new_spanned(
                        attribute,
                        "expected at least one guard function",
                    ));
                }
                guards.extend(paths.into_iter().map(|path| quote!(#path)));
            }
            "roles" | "permissions" => {
                let values =
                    attribute.parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated)?;
                if values.is_empty() || values.iter().any(|value| value.value().trim().is_empty()) {
                    return Err(syn::Error::new_spanned(
                        attribute,
                        "expected at least one non-empty role or permission",
                    ));
                }
                let function = if name == "roles" {
                    quote!(::xecute::auth::require_roles)
                } else {
                    quote!(::xecute::auth::require_permissions)
                };
                guards.push(quote!(|request| async move { #function(request, &[#values]).await }));
            }
            _ => retained.push(attribute),
        }
    }
    *attributes = retained;
    Ok(guards)
}

pub fn wrap(
    mut router: proc_macro2::TokenStream,
    guards: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    for guard in guards.iter().rev() {
        router = quote!(::xecute::guards::apply_routes(#router, #guard));
    }
    router
}
