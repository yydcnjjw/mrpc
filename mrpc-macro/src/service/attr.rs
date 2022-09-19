use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
    Ident, NestedMeta, Token,
};

pub enum IdentMeta {
    Ident(Ident),
    IdentMetaList(IdentMetaList),
}

impl IdentMeta {
    fn get_ident(&self) -> Ident {
        match self {
            IdentMeta::Ident(ident) => ident.clone(),
            IdentMeta::IdentMetaList(ml) => ml.ident.clone(),
        }
    }
}

impl Parse for IdentMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        parse_ident_meta_after_ident(ident, input)
    }
}

fn parse_ident_meta_after_ident(ident: Ident, input: ParseStream) -> syn::Result<IdentMeta> {
    if input.peek(token::Paren) {
        parse_ident_meta_list_after_ident(ident, input).map(IdentMeta::IdentMetaList)
    } else {
        Ok(IdentMeta::Ident(ident))
    }
}

fn parse_ident_meta_list_after_ident(
    ident: Ident,
    input: ParseStream,
) -> syn::Result<IdentMetaList> {
    let content;
    Ok(IdentMetaList {
        ident,
        paren_token: parenthesized!(content in input),
        list: content.parse_terminated(NestedMeta::parse)?,
    })
}

pub struct IdentMetaList {
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub list: Punctuated<NestedMeta, Token![,]>,
}

impl Parse for IdentMetaList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            ident: input.parse()?,
            paren_token: parenthesized!(content in input),
            list: content.parse_terminated(NestedMeta::parse)?,
        })
    }
}

pub struct MessageAttr {
    pub serde: Option<IdentMeta>,
    pub debug: Option<IdentMeta>,
}

impl MessageAttr {
    pub fn new() -> Self {
        Self {
            serde: None,
            debug: None,
        }
    }
}

pub fn set_only_none<T>(v: &mut Option<T>, set: T, span: Span) -> syn::Result<()> {
    match v {
        Some(_) => {
            return Err(syn::Error::new(span, "Duplicate identifier"));
        }
        None => {
            *v = Some(set);
        }
    };
    Ok(())
}

impl Parse for MessageAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attr = MessageAttr::new();

        let message: Ident = input.parse()?;
        if message != "message" {
            return Err(syn::Error::new(message.span(), "Need message ident"));
        }

        let content;
        parenthesized!(content in input);

        let ident_meta_vec = content.parse_terminated::<IdentMeta, Token![,]>(IdentMeta::parse)?;

        for ident_meta in ident_meta_vec {
            let ident = ident_meta.get_ident();
            match ident.to_string().as_str() {
                "debug" => {
                    set_only_none(&mut attr.debug, ident_meta, ident.span())?;
                }
                "serde" => {
                    set_only_none(&mut attr.serde, ident_meta, ident.span())?;
                }
                _ => {
                    return Err(syn::Error::new(ident.span(), "Unknown IdentMeta attr"));
                }
            }
        }
        Ok(attr)
    }
}

pub struct Attrs {
    message: Option<MessageAttr>,
}

impl Attrs {
    pub fn new() -> Self {
        Self { message: None }
    }

    pub fn gen_message_attr(&self) -> TokenStream2 {
        let mut token = TokenStream2::new();

        if let Some(attr) = &self.message {
            if let Some(_) = &attr.debug {
                token.extend(quote! {
                    #[derive(Debug)]
                });
            }

            if let Some(_) = &attr.serde {
                token.extend(quote! {
                    #[derive(mrpc::serde::Serialize,mrpc::serde::Deserialize)]
                    #[serde(crate = "mrpc::serde")]
                });
            }
        }

        token
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
