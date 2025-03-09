mod dependency;

use proc_macro::TokenStream;

#[proc_macro_derive(Dependency)]
pub fn dependency(input: TokenStream) -> TokenStream {
    dependency::dependency_impl(input)
}
