use proc_macro::TokenStream;
use quote::quote;
use syn::*;

#[proc_macro_derive(FromAny)]
pub fn from_any_derive(input: TokenStream) -> TokenStream {
    let ast = parse::<DeriveInput>(input).unwrap();
    let data = match &ast.data {
        Data::Enum(x) => x,
        _ => panic!("Only enum_dispatch types are supported."),
    };
    let fields = &data
        .variants
        .iter()
        .map(|x| match &x.fields {
            Fields::Unnamed(x) => x.unnamed.iter().next().unwrap(),
            _ => panic!("Only newtype variants are supported."),
        })
        .collect::<Vec<_>>();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    quote! {
        impl #impl_generics FromAny for #name #ty_generics #where_clause {
            fn from_any(x: impl std::any::Any) -> Option<Self> {
            let mut x = Some(x);
            let _x = &mut x as &mut dyn std::any::Any;
            #(
                if let Some(x) = _x.downcast_mut::<Option<#fields>>() {
                    std::mem::take(x).map(|x| x.into())
                } else
             )*
            {None}
            }
        }
    }
    .into()
}
