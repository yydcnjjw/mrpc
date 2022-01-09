use proc_macro::TokenStream;

mod common;
mod attr;
mod rpc;
mod server;
mod service;

#[proc_macro_attribute]
pub fn service(attr: TokenStream, input: TokenStream) -> TokenStream {
    service::parse(attr, input)
}

#[proc_macro_attribute]
pub fn server(attr: TokenStream, input: TokenStream) -> TokenStream {
    server::parse(attr, input)
}
