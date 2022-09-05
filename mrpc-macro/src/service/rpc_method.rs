use proc_macro2::Ident;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token, Attribute, FnArg, PatType, ReturnType, Token, Type,
};

pub struct RpcSignature {
    pub fn_token: Token![fn],
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub args: Punctuated<PatType, Token![,]>,
    pub output: Type,
}

impl Parse for RpcSignature {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            fn_token: input.parse()?,
            ident: input.parse()?,
            paren_token: parenthesized!(content in input),
            args: content.parse_terminated(|input| match FnArg::parse(input)? {
                FnArg::Receiver(arg) => Err(syn::Error::new(
                    arg.span(),
                    "method args cannot start with self",
                )),
                FnArg::Typed(arg) => match *arg.pat {
                    syn::Pat::Ident(_) => Ok(arg),
                    _ => Err(syn::Error::new(
                        arg.pat.span(),
                        "patterns aren't allowed in RPC args",
                    )),
                },
            })?,
            output: match ReturnType::parse(input)? {
                ReturnType::Default => parse_quote!(()),
                ReturnType::Type(_, ty) => *ty,
            },
        })
    }
}

pub struct RpcMethod {
    pub attrs: Vec<Attribute>,
    pub sig: RpcSignature,
}

impl Parse for RpcMethod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            sig: input.parse()?,
        })
    }
}
