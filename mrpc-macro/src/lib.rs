use proc_macro::TokenStream;

mod service;
mod service_impl;
mod router;

mod common;

#[proc_macro_attribute]
pub fn service(attr: TokenStream, input: TokenStream) -> TokenStream {
    service::parse(attr, input)
}

#[proc_macro_attribute]
pub fn service_impl(attr: TokenStream, input: TokenStream) -> TokenStream {
    service_impl::parse(attr, input)
}


#[proc_macro_attribute]
pub fn router(attr: TokenStream, input: TokenStream) -> TokenStream {
    router::parse(attr, input)
}
