use crate::common::*;
use convert_case::Case;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token, Attribute, AttributeArgs, FnArg, NestedMeta, ReturnType, Token, Type,
};
use syn::{PatType, Visibility};

#[allow(dead_code)]
struct RpcSignature {
    pub asyncness: Option<Token![async]>,
    pub fn_token: Token![fn],
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub inputs: Punctuated<PatType, Token![,]>,
    pub output: Type,
}

impl Parse for RpcSignature {
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
struct RpcMethod {
    pub attrs: Vec<Attribute>,
    pub sig: RpcSignature,
    pub semi_token: Option<Token![;]>,
}

impl Parse for RpcMethod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            sig: input.parse()?,
            semi_token: input.parse()?,
        })
    }
}

struct ServiceAttrs {
    pub derive_serde: bool,
}

impl ServiceAttrs {
    fn new() -> Self {
        Self {
            derive_serde: false,
        }
    }
}

impl From<AttributeArgs> for ServiceAttrs {
    fn from(attrs: AttributeArgs) -> Self {
        let mut self_ = Self::new();
        for attr in attrs {
            match attr {
                NestedMeta::Meta(attr) => match attr {
                    syn::Meta::Path(p) => {
                        let ident = p.get_ident();
                        if ident.is_none() {
                            continue;
                        }
                        let ident = ident.unwrap();
                        if ident == "serde" {
                            self_.derive_serde = true;
                        }
                    }
                    syn::Meta::NameValue(_) => {}
                    syn::Meta::List(_) => {}
                },
                NestedMeta::Lit(_) => {}
            }
        }
        self_
    }
}

#[allow(dead_code)]
struct Service {
    pub service_attrs: ServiceAttrs,
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub trait_token: Token![trait],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Vec<RpcMethod>,
}

impl Parse for Service {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            service_attrs: ServiceAttrs::new(),
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            trait_token: input.parse()?,
            ident: input.parse()?,
            brace_token: braced!(content in input),
            items: {
                let mut items = Vec::new();
                while !content.is_empty() {
                    items.push(content.parse()?);
                }
                items
            },
        })
    }
}

impl Service {
    fn rpc_idents(&self) -> Vec<&Ident> {
        self.items.iter().map(|rpc| &rpc.sig.ident).collect()
    }

    fn rpc_args(&self) -> Vec<Vec<&PatType>> {
        self.items
            .iter()
            .map(|rpc| rpc.sig.inputs.iter().collect())
            .collect()
    }

    fn rpc_return_types(&self) -> Vec<&Type> {
        self.items.iter().map(|rpc| &rpc.sig.output).collect()
    }

    fn request_ident(&self) -> Ident {
        format_ident!("{}Request", self.ident)
    }

    fn response_ident(&self) -> Ident {
        format_ident!("{}Response", self.ident)
    }

    fn request_item_idents(&self) -> Vec<Ident> {
        self.rpc_idents()
            .iter()
            .map(|ident| Self::request_item_ident(ident))
            .collect()
    }

    fn response_item_idents(&self) -> Vec<Ident> {
        self.rpc_idents()
            .iter()
            .map(|ident| Self::response_item_ident(ident))
            .collect()
    }

    fn request_item_ident(ident: &Ident) -> Ident {
        ident_to_case(ident, Case::UpperCamel)
    }

    fn response_item_ident(ident: &Ident) -> Ident {
        ident_to_case(ident, Case::UpperCamel)
    }

    fn client_ident(&self) -> Ident {
        format_ident!("{}Client", self.ident)
    }

    fn gen_derive_serde(&self) -> Option<TokenStream2> {
        if self.service_attrs.derive_serde {
            Some(quote! {
                #[derive(Debug, mrpc::serde::Serialize, mrpc::serde::Deserialize)]
                #[serde(crate = "mrpc::serde")]
            })
        } else {
            None
        }
    }

    fn gen_service(&self) -> TokenStream2 {
        let Self {
            service_attrs: _,
            attrs,
            vis,
            trait_token: _,
            ident,
            brace_token: _,
            items,
        } = self;

        let rpcs = items.iter().map(|RpcMethod { attrs, sig, .. }| {
            let RpcSignature {
                asyncness,
                fn_token: _,
                ident,
                paren_token: _,
                inputs,
                output,
            } = sig;

            let args = inputs.iter();

            quote! {
                #( #attrs )*
                #asyncness fn #ident(self: Arc<Self>,  #( #args ),*) -> #output;
            }
        });

        let fn_serve = {
            let request_ident = &self.request_ident();
            let response_ident = &self.response_ident();

            let match_items =
                items.iter().map(|RpcMethod { attrs: _, sig, .. }| {
                    let (
                        method_ident,
                        request_item_ident,
                        response_item_ident,
                        input_pats,
                        do_await,
                    ) = (
                        &sig.ident,
                        Self::request_item_ident(&sig.ident),
                        Self::response_item_ident(&sig.ident),
                        sig.inputs
                            .iter()
                            .map(|input| &*input.pat)
                            .collect::<Vec<_>>(),
                        sig.asyncness.map(|_| {
                            quote! { .await }
                        }),
                    );

                    let arg_pats = &input_pats;

                    quote! {
                        #request_ident::#request_item_ident{ #( #arg_pats ),* } => {
                            #response_ident::#response_item_ident(
                                Self::#method_ident(
                                    self, #( #arg_pats ),*
                                )#do_await
                            )
                        }
                    }
                });

            quote! {
                async fn serve(self: Arc<Self>, req: #request_ident) -> #response_ident {
                    match req {
                        #( #match_items )*
                    }
                }
            }
        };

        quote! {
            #[mrpc::async_trait]
            #( #attrs )*
            #vis trait #ident: Send + Sync {
                #( #rpcs )*

                #fn_serve
            }
        }
    }

    fn gen_request(&self) -> TokenStream2 {
        let (derive_serde, vis, request_ident, request_item_idents, rpc_args) = (
            self.gen_derive_serde(),
            &self.vis,
            self.request_ident(),
            self.request_item_idents(),
            self.rpc_args(),
        );

        quote! {
            #derive_serde
            #vis enum #request_ident {
                #( #request_item_idents{ #( #rpc_args ),* } ),*
            }
        }
    }

    fn gen_response(&self) -> TokenStream2 {
        let (derive_serde, vis, response_ident, response_item_idents, rpc_return_types) = (
            self.gen_derive_serde(),
            &self.vis,
            self.response_ident(),
            self.response_item_idents(),
            self.rpc_return_types(),
        );

        quote! {
            #[derive(Debug)]
            #derive_serde
            #vis enum #response_ident {
                #( #response_item_idents( #rpc_return_types ) ),*
            }
        }
    }

    fn gen_client(&self) -> TokenStream2 {
        let (vis, client_ident, request_ident, response_ident) = (
            &self.vis,
            self.client_ident(),
            self.request_ident(),
            self.response_ident(),
        );

        let rpcs = self.items.iter().map(|RpcMethod { attrs: _, sig, .. }| {
            let RpcSignature {
                asyncness: _,
                fn_token: _,
                ident,
                paren_token: _,
                inputs,
                output,
            } = sig;

            let args = inputs.iter();
            let arg_pats = inputs.iter().map(|input| &*input.pat).collect::<Vec<_>>();
            let request_item_ident = Self::request_item_ident(ident);
            let response_item_ident = Self::response_item_ident(ident);
            quote! {
                #vis async fn #ident(&self, #( #args ),*) -> mrpc::anyhow::Result<#output> {
                    let (tx, rx) = mrpc::tokio::sync::oneshot::channel::<#response_ident>();

                    self.poster.post(#request_ident::#request_item_ident{
                        #( #arg_pats ),*
                    }, tx).await?;

                    match rx.await? {
                        #response_ident::#response_item_ident(o) => {
                            Ok(o)
                        }
                        _ => {
                            Err(mrpc::anyhow::anyhow!("response not match require {}", stringify!(#response_item_ident)))
                        }
                    }
                }
            }
        });

        quote! {
            #vis struct #client_ident<Poster> {
                pub poster: Poster,
            }

            impl<Poster> #client_ident<Poster>
            where Poster: mrpc::Poster<#request_ident, #response_ident> {
                #( #rpcs )*
            }
        }
    }
}

impl ToTokens for Service {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_request(),
            self.gen_response(),
            self.gen_service(),
            self.gen_client(),
        ])
    }
}

pub fn parse(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut service = parse_macro_input!(input as Service);
    service.service_attrs = ServiceAttrs::from(parse_macro_input!(attrs as AttributeArgs));
    service.into_token_stream().into()
}
