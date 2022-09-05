use convert_case::{Case, Casing};
use proc_macro2::Ident;
use quote::format_ident;
use syn::Path;

pub fn ident_to_case(ident: &Ident, case: Case) -> Ident {
    format_ident!("{}", ident.to_string().to_case(case))
}

pub fn service_request_ident(service_ident: &Ident) -> Ident {
    format_ident!("{}Request", service_ident)
}

pub fn service_response_ident(service_ident: &Ident) -> Ident {
    format_ident!("{}Response", service_ident)
}

pub fn rpc_request_ident(rpc_ident: &Ident) -> Ident {
    ident_to_case(rpc_ident, Case::UpperCamel)
}

pub fn rpc_response_ident(rpc_ident: &Ident) -> Ident {
    ident_to_case(rpc_ident, Case::UpperCamel)
}

pub fn service_request_path(path: &Path) -> Path {
    let mut path = path.clone();
    let mut last = path.segments.last_mut().unwrap();
    last.ident = format_ident!("{}Request", last.ident);
    path
}

pub fn service_response_path(path: &Path) -> Path {
    let mut path = path.clone();
    let mut last = path.segments.last_mut().unwrap();
    last.ident = format_ident!("{}Response", last.ident);
    path
}
