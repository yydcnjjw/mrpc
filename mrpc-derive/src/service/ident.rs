use quote::format_ident;
use syn::Path;

pub fn service_request_ty(mut ty: Path) -> Path {
    let mut last = ty.segments.last_mut().unwrap();
    last.ident = format_ident!("{}Request", last.ident);
    ty
}

pub fn service_response_ty(mut ty: Path) -> Path {
    let mut last = ty.segments.last_mut().unwrap();
    last.ident = format_ident!("{}Response", last.ident);
    ty
}

pub fn service_api_ty(mut ty: Path) -> Path {
    let mut last = ty.segments.last_mut().unwrap();
    last.ident = format_ident!("{}Api", last.ident);
    ty
}
