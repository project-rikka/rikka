use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, FieldsNamed, Ident, Path, parse_quote};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = crate_path(input)?;
    let name = &input.ident;
    let (implemented, arguments, clause) = input.generics.split_for_impl();
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "Widget can only be derived for structs",
        ));
    };
    let (styles, shown, total) = match &data.fields {
        Fields::Named(fields) => (
            marked(fields, "style"),
            marked(fields, "debug"),
            fields.named.len(),
        ),
        Fields::Unit => (Vec::new(), Vec::new(), 0),
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "Widget can only be derived for unit structs or structs with named fields",
            ));
        }
    };
    let styled = match styles.as_slice() {
        [] => quote!(),
        [style] => quote! {
            impl #implemented #krate::Styled for #name #arguments #clause {
                fn style_mut(&mut self) -> &mut #krate::Style {
                    &mut self.#style
                }
            }
        },
        [_, second, ..] => {
            return Err(syn::Error::new_spanned(
                second,
                "only one field can be the widget's `#[style]`",
            ));
        }
    };
    let title = name.to_string();
    let finish = match shown.len() == total {
        true => quote!(finish()),
        false => quote!(finish_non_exhaustive()),
    };
    let labels = shown.iter().map(ToString::to_string);
    Ok(quote! {
        impl #implemented #krate::View for #name #arguments #clause {
            fn build(self) -> #krate::Element {
                #krate::Element::new(self)
            }
        }

        impl #implemented ::std::fmt::Debug for #name #arguments #clause {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.debug_struct(#title)
                    #(.field(#labels, &self.#shown))*
                    .#finish
            }
        }

        #styled
    })
}

fn marked<'a>(fields: &'a FieldsNamed, attribute: &str) -> Vec<&'a Ident> {
    fields
        .named
        .iter()
        .filter(|field| field.attrs.iter().any(|a| a.path().is_ident(attribute)))
        .filter_map(|field| field.ident.as_ref())
        .collect()
}

fn crate_path(input: &DeriveInput) -> syn::Result<Path> {
    let mut path: Path = parse_quote!(::rikka_ui);
    for attribute in input.attrs.iter().filter(|a| a.path().is_ident("widget")) {
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
