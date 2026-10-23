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
}
impl Parse for ModuleArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut result = Self {
            imports: Vec::new(),
            providers: Vec::new(),
            controllers: Vec::new(),
            exports: Vec::new(),
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
                _ => {
                    return Err(syn::Error::new_spanned(
                        field,
                        "expected imports, providers, controllers, or exports",
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
    let structure = parse_macro_input!(item as ItemStruct);
    let name = &structure.ident;
    let providers = args.providers;
    let imports = args.imports;
    let exports = args.exports;
    let _controllers = args.controllers;
    quote! {
        #structure
        impl #name {
            #[must_use]
            /// Return the unique module type name used for registration.
            pub fn module_name() -> &'static str { ::core::any::type_name::<Self>() }
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
