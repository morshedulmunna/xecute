use proc_macro::TokenStream;
use quote::quote;
use syn::{Fields, ItemStruct, LitStr, Type, parse_macro_input};

/// Registers a named-field type with XecuteAI's local dependency injector.
pub fn expand(item: TokenStream) -> TokenStream {
    expand_inner(item, true)
}

/// Generate only implementations when invoked as a derive macro.
pub fn derive(item: TokenStream) -> TokenStream {
    expand_inner(item, false)
}

fn expand_inner(item: TokenStream, include_structure: bool) -> TokenStream {
    let mut structure = parse_macro_input!(item as ItemStruct);
    let name = structure.ident.clone();
    let scope = match structure
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("scope"))
    {
        Some(attribute) => match attribute.parse_args::<LitStr>() {
            Ok(value) => match value.value().as_str() {
                "singleton" => quote!(::xecute::Scope::Singleton),
                "transient" => quote!(::xecute::Scope::Transient),
                "request" => quote!(::xecute::Scope::Request),
                _ => {
                    return syn::Error::new_spanned(
                        value,
                        "scope must be singleton, transient, or request",
                    )
                    .into_compile_error()
                    .into();
                }
            },
            Err(error) => return error.into_compile_error().into(),
        },
        None => quote!(::xecute::Scope::Singleton),
    };
    structure
        .attrs
        .retain(|attribute| !attribute.path().is_ident("scope"));
    let (legacy_fields, container_fields, dependencies) = {
        let Fields::Named(fields) = &mut structure.fields else {
            return syn::Error::new_spanned(&structure, "#[injectable] requires named fields")
                .into_compile_error()
                .into();
        };
        // Default-initialized state belongs to this provider and is not a DI dependency.
        let mut default_fields = Vec::new();
        for field in &fields.named {
            for attribute in &field.attrs {
                if attribute.path().is_ident("inject")
                    && !matches!(attribute.meta, syn::Meta::Path(_))
                {
                    match attribute.parse_args::<syn::Ident>() {
                        Ok(initializer) if initializer == "default" => {}
                        _ => {
                            return syn::Error::new_spanned(
                                attribute,
                                "use #[inject(default)] to initialize provider-owned state",
                            )
                            .into_compile_error()
                            .into();
                        }
                    };
                    default_fields.push(field.ident.clone().expect("named field"));
                }
            }
        }
        let legacy_fields = fields
            .named
            .iter_mut()
            .map(|field| {
                let ident = field.ident.as_ref().expect("named field");
                let ty = &field.ty;
                let value = field
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("inject"));
                field
                    .attrs
                    .retain(|attribute| !attribute.path().is_ident("inject"));
                if default_fields.contains(ident) {
                    quote!(#ident: ::core::default::Default::default())
                } else if value {
                    quote!(#ident: injector.value_of::<#ty>()?)
                } else if is_wrapper(ty, "Option") {
                    let dependency = inner_dependency_type(ty);
                    quote!(#ident: match injector.resolve::<::std::sync::Arc<#dependency>>() { Ok(value) => Some(value), Err(::xecute::InjectError::Missing(_)) => None, Err(error) => return Err(error) })
                } else {
                    quote!(#ident: injector.resolve::<#ty>()?)
                }
            })
            .collect::<Vec<_>>();
        let dependencies = fields
            .named
            .iter()
            .filter(|field| !default_fields.contains(field.ident.as_ref().expect("named field")))
            .map(|field| inner_dependency_type(&field.ty))
            .collect::<Vec<_>>();
        let container_fields = fields
            .named
            .iter()
            .map(|field| {
                let ident = field.ident.as_ref().expect("named field");
                let dependency = inner_dependency_type(&field.ty);
                if default_fields.contains(ident) {
                    quote!(#ident: ::core::default::Default::default())
                } else if is_wrapper(&field.ty, "Option") {
                    quote!(#ident: container.resolve_optional::<#dependency>()?)
                } else if is_wrapper(&field.ty, "Arc") {
                    quote!(#ident: container.resolve::<#dependency>()?)
                } else {
                    quote!(#ident: (*container.resolve::<#dependency>()?).clone())
                }
            })
            .collect::<Vec<_>>();
        (legacy_fields, container_fields, dependencies)
    };

    let declaration = include_structure.then_some(&structure);

    quote! {
        #declaration
        impl ::xecute::Injectable for #name {
            fn inject(injector: &::xecute::Injector) -> Result<Self, ::xecute::InjectError> {
                Ok(Self { #(#legacy_fields),* })
            }
        }
        impl ::xecute::Provider for #name {
            fn scope() -> ::xecute::Scope { #scope }
            fn dependencies() -> Vec<::xecute::ProviderKey> {
                vec![#(::xecute::ProviderKey::of::<#dependencies>()),*]
            }
            fn provide(container: &::xecute::Container) -> Result<Self, ::xecute::InjectError> {
                Ok(Self { #(#container_fields),* })
            }
        }
    }
    .into()
}

fn inner_dependency_type(ty: &Type) -> Type {
    let Type::Path(path) = ty else {
        return ty.clone();
    };
    let Some(segment) = path.path.segments.last() else {
        return ty.clone();
    };
    if (segment.ident == "Arc" || segment.ident == "Option")
        && let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner)) = arguments.args.first()
    {
        return inner_dependency_type(inner);
    }
    ty.clone()
}

fn is_wrapper(ty: &Type, wrapper: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == wrapper))
}
