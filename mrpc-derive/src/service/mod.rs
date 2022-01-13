mod attr;
mod item_enum;
mod item_impl;
mod item_trait;
mod rpc;
mod ident;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Token,
};

use self::{
    attr::Attrs as ServiceAttrs, item_enum::ItemEnum, item_impl::ItemImpl, item_trait::ItemTrait,
};

enum Service {
    Trait(ItemTrait),
    Enum(ItemEnum),
    Impl(ItemImpl),
}

impl Service {
    fn set_attrs(&mut self, attrs: ServiceAttrs) {
        match self {
            Service::Trait(v) => v.attrs = attrs,
            Service::Enum(v) => v.attrs = attrs,
            _ => {}
        }
    }
}

impl Parse for Service {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![trait]) {
            Ok(Service::Trait(input.parse()?))
        } else if input.peek(Token![enum]) {
            Ok(Service::Enum(input.parse()?))
        } else if input.peek(Token![impl]) {
            Ok(Service::Impl(input.parse()?))
        } else {
            return Err(syn::Error::new(
                input.span(),
                "#[mrpc::service] only supports trait, enum, impl",
            ));
        }
    }
}

impl ToTokens for Service {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            Self::Trait(v) => v.to_tokens(tokens),
            Self::Enum(v) => v.to_tokens(tokens),
            Self::Impl(v) => v.to_tokens(tokens),
        }
    }
}

pub fn parse(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut service = parse_macro_input!(input as Service);
    service.set_attrs(parse_macro_input!(attrs as ServiceAttrs));
    service.into_token_stream().into()
}
