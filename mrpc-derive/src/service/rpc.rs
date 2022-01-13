use proc_macro2::Ident;
use syn::token::Paren;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token, Attribute, FnArg, PatType, ReturnType, Token, Type,
};

use super::attr::{set_only_none, MessageAttr};

#[allow(dead_code)]
pub struct Signature {
    pub asyncness: Option<Token![async]>,
    pub fn_token: Token![fn],
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub inputs: Punctuated<PatType, Token![,]>,
    pub output: Type,
}

impl Parse for Signature {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            asyncness: input.parse()?,
            fn_token: input.parse()?,
            ident: input.parse()?,
            paren_token: parenthesized!(content in input),
            inputs: content.parse_terminated(|input| match FnArg::parse(input)? {
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

#[allow(dead_code)]
pub struct Method {
    pub attrs: Attrs,
    pub sig: Signature,
    pub semi_token: Option<Token![;]>,
}

impl Parse for Method {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut rpc_attrs = Attrs::new();
        let attrs = input.call(Attribute::parse_outer)?;
        if attrs.len() > 1 {
            return Err(syn::Error::new(input.span(), "Expect one attr"));
        }

        if let Some(attr) = attrs.first() {
            if let Some(ident) = attr.path.get_ident() {
                if ident != "rpc" {
                    return Err(syn::Error::new(attr.span(), "Expect rpc attr"));
                } else {
                    let ParenRpcAttrs {
                        paren_token: _,
                        inner,
                    } = syn::parse2::<ParenRpcAttrs>(attr.tokens.clone())?;
                    rpc_attrs = inner;
                }
            } else {
                return Err(syn::Error::new(attr.span(), "Expect rpc attr"));
            }
        }

        Ok(Self {
            attrs: rpc_attrs,
            sig: input.parse()?,
            semi_token: input.parse()?,
        })
    }
}

pub struct Attrs {
    pub message: Option<MessageAttr>,
}

impl Attrs {
    fn new() -> Self {
        Self { message: None }
    }
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attr_vec = input.parse_terminated::<MessageAttr, Token![,]>(MessageAttr::parse)?;

        let mut message = None;

        for attr in attr_vec {
            set_only_none(&mut message, attr, input.span())?;
        }

        Ok(Self { message })
    }
}

#[allow(dead_code)]
struct ParenRpcAttrs {
    paren_token: Paren,
    inner: Attrs,
}

impl Parse for ParenRpcAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            paren_token: parenthesized!(content in input),
            inner: content.parse()?,
        })
    }
}
