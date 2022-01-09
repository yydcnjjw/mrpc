use crate::{
    attr::{set_only_none, IdentMeta, MessageAttr},
    common::*,
    rpc::{RpcAttrs, RpcMethod, RpcSignature},
};
use convert_case::Case;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::Visibility;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input, token, Token,
};

struct ServiceAttrs {
    message: Option<MessageAttr>,
}

impl ServiceAttrs {
    fn new() -> Self {
        Self { message: None }
    }

    fn gen_message_attr(&self) -> TokenStream2 {
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

impl Parse for ServiceAttrs {
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
struct Service {
    pub service_attrs: ServiceAttrs,
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
    fn request_ident(&self) -> Ident {
        format_ident!("{}Request", self.ident)
    }

    fn request_item_ident(rpc_ident: &Ident) -> Ident {
        ident_to_case(rpc_ident, Case::UpperCamel)
    }

    fn response_ident(&self) -> Ident {
        format_ident!("{}Response", self.ident)
    }

    fn response_item_ident(rpc_ident: &Ident) -> Ident {
        ident_to_case(rpc_ident, Case::UpperCamel)
    }

    fn client_ident(&self) -> Ident {
        format_ident!("{}Client", self.ident)
    }

    fn poster_ident(&self) -> Ident {
        format_ident!("{}Poster", self.ident)
    }

    fn gen_service(&self) -> TokenStream2 {
        let Self {
            service_attrs: _,
            vis,
            trait_token: _,
            ident,
            brace_token: _,
            items,
        } = self;

        let rpcs = items.iter().map(|RpcMethod { attrs: _, sig, .. }| {
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
                #asyncness fn #ident(self: std::sync::Arc<Self>,  #( #args ),*) -> #output;
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
                async fn serve(self: std::sync::Arc<Self>, req: #request_ident) -> #response_ident {
                    match req {
                        #( #match_items )*
                    }
                }
            }
        };

        quote! {
            #[mrpc::async_trait]
            #vis trait #ident: Send + Sync {
                #( #rpcs )*

                #fn_serve
            }
        }
    }

    fn gen_message_item_attr(attrs: &RpcAttrs) -> TokenStream2 {
        let mut attr = TokenStream2::new();
        if let Some(message) = &attrs.message {
            if let Some(serde) = &message.serde {
                if let IdentMeta::IdentMetaList(ml) = serde {
                    let token = ml.list.to_token_stream();
                    attr.extend(quote! {
                        #[serde(#token)]
                    })
                }
            }
        }

        attr
    }

    fn gen_request(&self) -> TokenStream2 {
        let (message_attr, vis, request_ident) = (
            self.service_attrs.gen_message_attr(),
            &self.vis,
            self.request_ident(),
        );

        let items = self.items.iter().map(|RpcMethod { attrs, sig, .. }| {
            let (request_item_ident, args) =
                (Self::request_item_ident(&sig.ident), sig.inputs.iter());

            let attr = Self::gen_message_item_attr(attrs);

            quote! {
                #attr
                #request_item_ident{ #( #args ),* }
            }
        });

        quote! {
            #message_attr
            #vis enum #request_ident {
                #( #items ),*
            }
        }
    }

    fn gen_response(&self) -> TokenStream2 {
        let (message_attr, vis, response_ident) = (
            self.service_attrs.gen_message_attr(),
            &self.vis,
            self.response_ident(),
        );

        let items = self.items.iter().map(|RpcMethod { attrs, sig, .. }| {
            let (response_item_ident, return_type) =
                (Self::response_item_ident(&sig.ident), &sig.output);

            let attr = Self::gen_message_item_attr(attrs);

            quote! {
                #attr
                #response_item_ident( #return_type )
            }
        });

        quote! {
            #message_attr
            #vis enum #response_ident {
                #( #items ),*
            }
        }
    }

    fn gen_client(&self) -> TokenStream2 {
        let (vis, client_ident, request_ident, response_ident, poster_ident) = (
            &self.vis,
            self.client_ident(),
            self.request_ident(),
            self.response_ident(),
            self.poster_ident(),
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
                    let (tx, rx) = mrpc::sync::oneshot::channel::<#response_ident>();

                    self.poster.post(#request_ident::#request_item_ident{
                        #( #arg_pats ),*
                    }, tx).await?;

                    match rx.await? {
                        #response_ident::#response_item_ident(o) => {
                            Ok(o)
                        }
                        _ => {
                            Err(mrpc::anyhow::anyhow!("response not match require {}",
                                                      stringify!(#response_item_ident)))
                        }
                    }
                }
            }
        });

        quote! {
            #[derive(Clone)]
            #vis struct #client_ident<Poster> {
                pub poster: Poster,
            }

            impl<Poster> #client_ident<Poster>
            where Poster: #poster_ident {
                #( #rpcs )*
            }
        }
    }

    fn gen_poster(&self) -> TokenStream2 {
        let (vis, poster_ident, request_ident, response_ident) = (
            &self.vis,
            self.poster_ident(),
            self.request_ident(),
            self.response_ident(),
        );
        quote! {
            #vis trait #poster_ident: mrpc::Poster<#request_ident, #response_ident> + Clone + Sync + Send {}
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
            self.gen_poster(),
        ])
    }
}

pub fn parse(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut service = parse_macro_input!(input as Service);
    service.service_attrs = parse_macro_input!(attrs as ServiceAttrs);
    service.into_token_stream().into()
}
