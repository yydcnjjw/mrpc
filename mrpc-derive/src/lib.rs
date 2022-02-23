use proc_macro::TokenStream;

mod service;

mod common;

#[proc_macro_attribute]
pub fn service(attr: TokenStream, input: TokenStream) -> TokenStream {
    service::parse(attr, input)
}
