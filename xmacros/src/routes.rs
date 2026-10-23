use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    ItemFn, LitStr, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

pub enum Method {
    Get,
    Post,
    Patch,
    Delete,
    Put,
    Head,
    Options,
}
struct RouteArgs {
    path: LitStr,
    state: Type,
}
impl Parse for RouteArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let path = input.parse()?;
        if input.is_empty() {
            return Ok(Self {
                path,
                state: syn::parse_quote!(crate::AppState),
            });
        }
        input.parse::<Token![,]>()?;
        let name: syn::Ident = input.parse()?;
        if name != "state" {
            return Err(syn::Error::new_spanned(
                name,
                "expected `state = StateType`",
            ));
        }
        input.parse::<Token![=]>()?;
        Ok(Self {
            path,
            state: input.parse()?,
        })
    }
}
pub fn expand(attribute: TokenStream, item: TokenStream, method: Method) -> TokenStream {
    let RouteArgs { path, state } = parse_macro_input!(attribute as RouteArgs);
    let handler = parse_macro_input!(item as ItemFn);
    let name = &handler.sig.ident;
    let route = format_ident!("__xecuteai_route_{name}");
    let method = match method {
        Method::Get => quote!(get),
        Method::Post => quote!(post),
        Method::Patch => quote!(patch),
        Method::Delete => quote!(delete),
        Method::Put => quote!(put),
        Method::Head => quote!(head),
        Method::Options => quote!(options),
    };
    quote! { #handler #[doc(hidden)] pub fn #route() -> ::axum::Router<#state> { ::axum::Router::new().route(#path, ::axum::routing::#method(#name)) } }.into()
}
