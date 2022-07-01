use proc_macro::TokenStream;
use quote::quote;
use syn::*;

#[proc_macro_derive(EnumCast)]
pub fn enum_cast_derive(input: TokenStream) -> TokenStream {
    let ast = parse::<DeriveInput>(input).unwrap();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    match &ast.data {
        Data::Enum(data) => {
            let fields = &data
                .variants
                .iter()
                .map(|x| match &x.fields {
                    Fields::Unnamed(x) => x.unnamed.iter().next().unwrap(),
                    _ => panic!("Only newtype variants are supported."),
                })
                .collect::<Vec<_>>();

            let idents = &data.variants.iter().map(|x| &x.ident).collect::<Vec<_>>();

            quote! {
                #(
                    impl #impl_generics EnumRef<#fields> for  #name #ty_generics #where_clause {
                        fn enum_ref(&self) -> Option<&#fields> {
                            match self {
                                Self::#idents(x) => Some(x),
                                _ => None,
                            }
                        }
                        fn enum_mut(&mut self) -> Option<&mut #fields> {
                            match self {
                                Self::#idents(x) => Some(x),
                                _ => None,
                            }
                        }
                    }
                )*

                impl #impl_generics EnumCast for #name #ty_generics #where_clause {
                    fn cast<T: 'static>(&self) -> Option<&T> {
                        match self {
                            #(
                                Self::#idents(x) => x as &dyn std::any::Any
                            ),*
                        }.downcast_ref::<T>()
                    }

                    fn cast_mut<T: 'static>(&mut self) -> Option<&mut T> {
                        match self {
                            #(
                                Self::#idents(x) => x as &mut dyn std::any::Any
                            ),*
                        }.downcast_mut::<T>()
                    }

                    fn from_any<T: 'static>(x: T) -> Option<Self> {
                        let mut x = Some(x);
                        let x = &mut x as &mut dyn std::any::Any;
                        #(
                            if let Some(x) = x.downcast_mut::<Option<#fields>>() {
                                std::mem::take(x).map(Self::#idents)
                            } else
                         )*
                        {None}
                    }
                }
            }
        }
        Data::Struct(_) => {
            quote! {
                impl #impl_generics EnumCast for #name #ty_generics #where_clause {
                    fn cast<T: 'static>(&self) -> Option<&T> {
                        (self as &dyn std::any::Any).downcast_ref::<T>()
                    }

                    fn cast_mut<T: 'static>(&mut self) -> Option<&mut T> {
                        (self as &mut dyn std::any::Any).downcast_mut::<T>()
                    }
                    fn from_any<T: 'static>(x: T) -> Option<Self> {
                        let mut x = Some(x);
                        let x = &mut x as &mut dyn std::any::Any;
                        x.downcast_mut::<Option<Self>>().and_then(std::mem::take)
                    }
                }

            }
        }
        _ => panic!("Only enum_dispatch or struct types are supported."),
    }
    .into()
}
