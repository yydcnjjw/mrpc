use convert_case::{Case, Casing};
use proc_macro2::Ident;
use quote::format_ident;

pub fn ident_to_case(ident: &Ident, case: Case) -> Ident {
    format_ident!("{}", ident.to_string().to_case(case))
}
