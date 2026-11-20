mod children;
mod widget;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Path, parse_macro_input, parse_quote};

#[proc_macro]
pub fn children(input: TokenStream) -> TokenStream {
    match children::expand(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(Widget, attributes(widget, style, debug))]
pub fn derive_widget(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match widget::expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(State, attributes(state, untracked))]
pub fn derive_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Nested,
    Tracked,
    Untracked,
    Plain,
}

struct Field {
    access: Tokens,
    ty: syn::Type,
    kind: Kind,
}

fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    let krate = crate_path(&input.attrs)?;
    let name = &input.ident;
    let visibility = &input.vis;
    let mut generics = input.generics.clone();
    for parameter in generics.type_params_mut() {
        parameter.bounds.push(parse_quote!('static));
    }
    let (implemented, arguments, clause) = generics.split_for_impl();
    let (holds, reach, plain, nested) = match &input.data {
        Data::Struct(data) => {
            let fields = fields(&data.fields, |access| quote!(self.#access))?;
            let plain = fields.iter().any(|field| field.kind == Kind::Plain);
            (
                holding(&krate, &fields),
                reaching(&krate, &fields),
                quote!(#plain),
                nesting(&krate, &fields),
            )
        }
        Data::Enum(data) => {
            let mut holds = Vec::new();
            let mut reach = Vec::new();
            for variant in &data.variants {
                let ident = &variant.ident;
                let bound = fields(&variant.fields, |access| {
                    let binding = format_ident!("__field_{}", access.to_string());
                    quote!((*#binding))
                })?;
                let bindings = bindings(&variant.fields);
                let pattern = match &variant.fields {
                    Fields::Named(_) => quote!(Self::#ident { #(#bindings),* }),
                    Fields::Unnamed(_) => quote!(Self::#ident(#(#bindings),*)),
                    Fields::Unit => quote!(Self::#ident),
                };
                let reach_count = holding(&krate, &bound);
                let reached = reaching(&krate, &bound);
                holds.push(quote!(#pattern => { #reach_count }));
                reach.push(quote!(#pattern => { #reached }));
            }
            (
                quote!(match self { #(#holds)* }),
                quote!(match self { #(#reach)* }),
                quote!(true),
                quote!(false),
            )
        }
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "State can only be derived for structs and enums",
            ));
        }
    };
    Ok(quote! {
        impl #implemented #name #arguments #clause {
            #[doc(hidden)]
            #visibility const HAS_PLAIN: bool = #plain;
        }

        impl #implemented #krate::State for #name #arguments #clause {
            fn holds(&self, target: ::std::any::TypeId) -> usize {
                let mut reach_count = usize::from(::std::any::TypeId::of::<Self>() == target);
                reach_count += { #holds };
                reach_count
            }

            fn reach(&mut self, target: ::std::any::TypeId) -> ::std::option::Option<#krate::StateRef<'_>> {
                if ::std::any::TypeId::of::<Self>() == target {
                    let plain = #krate::State::has_plain(self);
                    return ::std::option::Option::Some(#krate::StateRef::new(self, plain));
                }
                #reach
            }

            fn has_plain(&self) -> bool {
                Self::HAS_PLAIN || #nested
            }
        }
    })
}

fn crate_path(attributes: &[syn::Attribute]) -> syn::Result<Path> {
    let mut path: Path = parse_quote!(::rikka_ui);
    for attribute in attributes.iter().filter(|a| a.path().is_ident("state")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let named: syn::LitStr = meta.value()?.parse()?;
                path = named.parse()?;
                Ok(())
            } else {
                Err(meta.error("expected `crate = \"path\"`"))
            }
        })?;
    }
    Ok(path)
}

fn fields(fields: &Fields, access: impl Fn(Tokens) -> Tokens) -> syn::Result<Vec<Field>> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let member = match &field.ident {
                Some(ident) => quote!(#ident),
                None => {
                    let index = syn::Index::from(index);
                    quote!(#index)
                }
            };
            Ok(Field {
                access: access(member),
                ty: field.ty.clone(),
                kind: kind(field)?,
            })
        })
        .collect()
}

fn kind(field: &syn::Field) -> syn::Result<Kind> {
    let mut kind = None;
    for attribute in &field.attrs {
        let marked = match attribute.path() {
            path if path.is_ident("state") => Kind::Nested,
            path if path.is_ident("untracked") => Kind::Untracked,
            _ => continue,
        };
        if !matches!(attribute.meta, syn::Meta::Path(_)) {
            return Err(syn::Error::new_spanned(
                &attribute.meta,
                "expected a bare `#[state]` or `#[untracked]`",
            ));
        }
        if kind
            .replace(marked)
            .is_some_and(|earlier| earlier != marked)
        {
            return Err(syn::Error::new_spanned(
                attribute,
                "a field is either `#[state]` or `#[untracked]`",
            ));
        }
    }
    Ok(kind.unwrap_or_else(|| match tracked(&field.ty) {
        true => Kind::Tracked,
        false => Kind::Plain,
    }))
}

fn tracked(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.path.segments.last().is_some_and(|segment| {
        segment.ident == "Tracked" || segment.ident == "TrackedVec" || segment.ident == "Revision"
    })
}

fn bindings(fields: &Fields) -> Vec<Tokens> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| match &field.ident {
            Some(ident) => {
                let binding = format_ident!("__field_{}", ident);
                quote!(#ident: #binding)
            }
            None => {
                let binding = format_ident!("__field_{}", index);
                quote!(#binding)
            }
        })
        .collect()
}

fn holding(krate: &Path, fields: &[Field]) -> Tokens {
    let counted = fields.iter().map(|field| {
        let Field { access, ty, kind } = field;
        match kind {
            Kind::Nested => quote!(#krate::State::holds(&#access, target)),
            _ => quote!(usize::from(::std::any::TypeId::of::<#ty>() == target)),
        }
    });
    quote!(0usize #(+ #counted)*)
}

fn reaching(krate: &Path, fields: &[Field]) -> Tokens {
    let tried = fields.iter().map(|field| {
        let Field { access, ty, kind } = field;
        let plain = *kind == Kind::Plain;
        match kind {
            Kind::Nested => quote! {
                if #krate::State::holds(&#access, target) > 0 {
                    return #krate::State::reach(&mut #access, target);
                }
            },
            _ => quote! {
                if ::std::any::TypeId::of::<#ty>() == target {
                    return ::std::option::Option::Some(#krate::StateRef::new(&mut #access, #plain));
                }
            },
        }
    });
    quote! {
        #(#tried)*
        ::std::option::Option::None
    }
}

fn nesting(krate: &Path, fields: &[Field]) -> Tokens {
    let nested = fields
        .iter()
        .filter(|field| field.kind == Kind::Nested)
        .map(|Field { access, .. }| quote!(#krate::State::has_plain(&#access)));
    quote!(false #(|| #nested)*)
}
