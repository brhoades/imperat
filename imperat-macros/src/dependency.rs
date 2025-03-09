use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

pub fn dependency_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::imperat::FromTypeMap for #name #type_generics #where_clause {
            fn retrieve_from_map(tm: &::imperat::TypeMap) -> Option<Self> {
                tm.get::<Self>().cloned()
            }
        }
    }
    .into()
}
