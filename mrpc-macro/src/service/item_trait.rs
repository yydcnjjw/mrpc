use convert_case::Case;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    token, Ident, Token, Visibility,
};

use crate::common::ident_to_case;

use super::{attr::Attrs, attr::IdentMeta, rpc};

pub struct ItemTrait {
    pub attrs: Attrs,
    pub vis: Visibility,
    pub trait_token: Token![trait],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Vec<rpc::Method>,
}

impl Parse for ItemTrait {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attrs: Attrs::new(),
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

impl ItemTrait {
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

    fn service_ident(&self) -> Ident {
        self.ident.clone()
    }

    fn shared_service_ident(&self) -> Ident {
        format_ident!("Shared{}", self.service_ident())
    }

    fn api_ident(&self) -> Ident {
        format_ident!("{}Api", self.ident)
    }

    fn client_ident(&self) -> Ident {
        format_ident!("{}Client", self.ident)
    }

    fn gen_service_serve(&self) -> TokenStream2 {
        let (items, request_ident, response_ident) =
            (&self.items, self.request_ident(), self.response_ident());

        let match_items = items.iter().map(|rpc::Method { attrs: _, sig, .. }| {
            let (method_ident, request_item_ident, response_item_ident, input_pats, do_await) = (
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
            async fn serve(self: mrpc::sync::Arc<Self>,
                           req: #request_ident)
                           -> mrpc::anyhow::Result<#response_ident> {
                Ok(match req {
                    #( #match_items )*
                })
            }
        }
    }

    fn gen_service(&self) -> TokenStream2 {
        let (vis, service_ident, items, shared_service_ident, request_ident, response_ident) = (
            &self.vis,
            self.service_ident(),
            &self.items,
            self.shared_service_ident(),
            self.request_ident(),
            self.response_ident(),
        );

        let rpcs = items.iter().map(|rpc::Method { attrs: _, sig, .. }| {
            let rpc::Signature {
                asyncness,
                fn_token: _,
                ident,
                paren_token: _,
                inputs,
                output,
            } = sig;

            let args = inputs.iter();

            quote! {
                #asyncness fn #ident(self: mrpc::sync::Arc<Self>,  #( #args ),*) -> #output;
            }
        });

        let fn_serve = self.gen_service_serve();

        quote! {
            #[mrpc::async_trait]
            #vis trait #service_ident: Send + Sync {
                #( #rpcs )*

                #fn_serve
            }

            #vis type #shared_service_ident = mrpc::SharedService<#request_ident, #response_ident>;
        }
    }

    fn gen_message_item_attr(attrs: &rpc::Attrs) -> TokenStream2 {
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
            self.attrs.gen_message_attr(),
            &self.vis,
            self.request_ident(),
        );

        let items = self.items.iter().map(|rpc::Method { attrs, sig, .. }| {
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
            self.attrs.gen_message_attr(),
            &self.vis,
            self.response_ident(),
        );

        let items = self.items.iter().map(|rpc::Method { attrs, sig, .. }| {
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

    fn gen_api(&self) -> TokenStream2 {
        let (vis, api_ident, request_ident, response_ident, client_ident) = (
            &self.vis,
            self.api_ident(),
            self.request_ident(),
            self.response_ident(),
            self.client_ident(),
        );

        let rpcs = self.items.iter().map(|rpc::Method { attrs: _, sig, .. }| {
            let rpc::Signature {
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

                    let response = self.sender.send_request(
                        #request_ident::#request_item_ident{
                            #( #arg_pats ),*
                        }).await;

                    response.and_then(|v| match v {
                        #response_ident::#response_item_ident(o) => {
                            Ok(o)
                        }
                        _ => {
                            Err(mrpc::anyhow::Error::msg(
                                format!("response not match require {}",
                                        stringify!(#response_item_ident))))
                        }
                    })
                }
            }
        });

        quote! {
            #[derive(Clone)]
            #vis struct #api_ident {
                sender: mrpc::client::Sender<#request_ident, #response_ident>
            }

            impl #api_ident {
                #( #rpcs )*
            }

            impl mrpc::client::Api for #api_ident {
                type Request = #request_ident;
                type Response = #response_ident;

                fn create(sender: mrpc::client::Sender<
                        #request_ident, #response_ident
                    >) -> Self {
                    Self {
                        sender,
                    }
                }

                fn sender(&self) -> mrpc::client::Sender<
                        #request_ident, #response_ident
                    > {
                    self.sender.clone()
                }
            }

            #vis type #client_ident = mrpc::Client<#api_ident>;
        }
    }
}

impl ToTokens for ItemTrait {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_request(),
            self.gen_response(),
            self.gen_service(),
            self.gen_api(),
        ])
    }
}
