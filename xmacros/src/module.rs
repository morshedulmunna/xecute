use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, TypePath, bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

struct ModuleArgs {
    imports: Vec<TypePath>,
    providers: Vec<TypePath>,
    controllers: Vec<TypePath>,
    exports: Vec<TypePath>,
    guards: Vec<TypePath>,
}
impl Parse for ModuleArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut result = Self {
            imports: Vec::new(),
            providers: Vec::new(),
            controllers: Vec::new(),
            exports: Vec::new(),
            guards: Vec::new(),
        };
        while !input.is_empty() {
            let field: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let content;
            bracketed!(content in input);
            let values = content
                .parse_terminated(TypePath::parse, Token![,])?
                .into_iter()
                .collect();
            match field.to_string().as_str() {
                "imports" => result.imports = values,
                "providers" => result.providers = values,
                "controllers" => result.controllers = values,
                "exports" => result.exports = values,
                "guards" => {
                    if values.is_empty() {
                        return Err(syn::Error::new_spanned(
                            field,
                            "expected at least one guard function",
                        ));
                    }
                    result.guards.extend(values);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        field,
                        "expected imports, providers, controllers, exports, or guards",
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(result)
    }
}

/// Generates module metadata and provider registrations for `injectable`.
pub fn expand(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attribute as ModuleArgs);
    let mut structure = parse_macro_input!(item as ItemStruct);
    let mut guards: Vec<_> = args.guards.iter().map(|guard| quote!(#guard)).collect();
    match crate::guards::take(&mut structure.attrs) {
        Ok(attributes) => guards.extend(attributes),
        Err(error) => return error.into_compile_error().into(),
    }
    let guarded = crate::guards::wrap(quote!(router), &guards);
    let name = &structure.ident;
    let providers = args.providers;
    let imports = args.imports;
    let exports = args.exports;
    let controllers = args.controllers;
    quote! {
        #structure
        impl #name {
            #[must_use]
            /// Return the unique module type name used for registration.
            pub fn module_name() -> &'static str { ::core::any::type_name::<Self>() }
            #[must_use]
            /// Apply this module's declared guards to its assembled routes.
            /// Call before merging unrelated modules; install request context outside this wrapper.
            pub fn apply_guards(router: ::xecute::prelude::Router) -> ::xecute::prelude::Router {
                #guarded
            }
            #[must_use]
            /// Assemble declared controllers with module guards and request-scoped DI context.
            /// Imported modules require their own explicit route assembly.
            pub fn scoped_router(container: ::std::sync::Arc<::xecute::Container>) -> ::xecute::prelude::Router {
                let router = ::xecute::prelude::Router::new();
                #(let router = router.merge(<#controllers>::scoped_router(container.clone()));)*
                ::xecute::with_container(Self::apply_guards(router), container)
            }
            #[must_use]
            /// Describe this module, its providers, imports, and exports.
            pub fn definition() -> ::xecute::ModuleDefinition {
                let module = ::xecute::ModuleDefinition::new(Self::module_name());
                #(let module = module.import(<#imports>::module_name());)*
                #(let module = module.provider(<#providers as ::xecute::Provider>::definition());)*
                #(let module = module.export::<#exports>();)*
                module
            }
        }
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_module_guard_paths_in_order_and_rejects_empty_declarations() {
        let args = syn::parse_str::<ModuleArgs>(
            "providers = [Service], controllers = [Controller], guards = [auth::verify, require_admin]",
        ).unwrap();
        let paths: Vec<_> = args
            .guards
            .iter()
            .map(|path| quote!(#path).to_string())
            .collect();
        assert_eq!(paths, ["auth :: verify", "require_admin"]);
        assert!(syn::parse_str::<ModuleArgs>("guards = []").is_err());
        assert!(syn::parse_str::<ModuleArgs>(r#"guards = ["invalid"]"#).is_err());
    }

    #[test]
    fn module_policy_annotations_use_the_same_validation_as_controller_policies() {
        for annotation in ["#[guard()]", "#[roles()]", "#[permissions(\"\")]"] {
            let mut item: ItemStruct =
                syn::parse_str(&format!("{annotation} struct Module;")).unwrap();
            assert!(crate::guards::take(&mut item.attrs).is_err());
        }
    }
}
