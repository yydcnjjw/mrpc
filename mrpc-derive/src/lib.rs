use proc_macro::TokenStream;

mod service;

mod common;

#[proc_macro_attribute]
pub fn service(attr: TokenStream, input: TokenStream) -> TokenStream {
    service::parse(attr, input)
}

// #[proc_macro_derive(Service)]
// pub fn derive_service(input: TokenStream) -> TokenStream {
    
// }
