use convert_case::Case;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Attribute, Path, Token, Visibility,
};

use crate::common::{self, ident_to_case};

pub struct RouterItem {
    pub attrs: Vec<Attribute>,
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub path: Punctuated<Path, Token![,]>,
}

impl Parse for RouterItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            ident: input.parse()?,
            paren_token: parenthesized!(content in input),
            path: content.parse_terminated(Path::parse)?,
        })
    }
}

pub struct Router {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub enum_token: Token![enum],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Punctuated<RouterItem, Token![,]>,
}

impl Router {
    fn gen_router_request_response(&self) -> TokenStream2 {
        let (attrs, vis, service_request_ident, service_response_ident) = (
            &self.attrs,
            &self.vis,
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
        );

        let (request_items, response_items): (Vec<_>, Vec<_>) = self
            .items
            .iter()
            .map(
                |RouterItem {
                     attrs,
                     ident,
                     paren_token: _,
                     path,
                 }| {
                    let sub_service_path = path.first().unwrap();
                    let sub_service_request_path = common::service_request_path(sub_service_path);
                    let sub_service_response_path = common::service_response_path(sub_service_path);
                    (
                        quote! {
                            #( #attrs )*
                            #ident( #sub_service_request_path )
                        },
                        quote! {
                            #( #attrs )*
                            #ident( #sub_service_response_path )
                        },
                    )
                },
            )
            .unzip();

        quote! {
            #[derive(mrpc::serde::Serialize, mrpc::serde::Deserialize, Debug)]
            #[serde(crate = "mrpc::serde")]
            #( #attrs )*
            #vis enum #service_request_ident {
                #( #request_items ),*
            }

            #[derive(mrpc::serde::Serialize, mrpc::serde::Deserialize, Debug)]
            #[serde(crate = "mrpc::serde")]
            #( #attrs )*
            #vis enum #service_response_ident {
                #( #response_items ),*
            }
        }
    }

    fn gen_item_convert(&self, sub_service_ident: &Ident, sub_service_path: &Path) -> TokenStream2 {
        let (
            service_request_ident,
            service_response_ident,
            sub_service_request_path,
            sub_service_response_path,
        ) = (
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
            common::service_request_path(sub_service_path),
            common::service_response_path(sub_service_path),
        );

        quote! {
            impl From<#sub_service_request_path> for #service_request_ident {
                fn from(req: #sub_service_request_path) -> Self {
                    Self::#sub_service_ident(req)
                }
            }

            impl TryFrom<#service_response_ident> for #sub_service_response_path {
                type Error = mrpc::AnyError;
                fn try_from(resp: #service_response_ident) -> Result<Self, Self::Error> {
                    match resp {
                        #service_response_ident::#sub_service_ident(resp) => Ok(resp),
                        _ => mrpc::bail!("expect {}, but get {:?} actually ", stringify!(#sub_service_ident), resp),
                    }
                }
            }
        }
    }

    fn gen_convert(&self) -> TokenStream2 {
        let items = self.items.iter().map(
            |RouterItem {
                 attrs: _,
                 ident,
                 paren_token: _,
                 path,
             }| self.gen_item_convert(ident, path.first().unwrap()),
        );

        quote! {
            #( #items )*
        }
    }

    fn router_trait_ident(&self) -> Ident {
        format_ident!("{}Router", self.ident)
    }

    fn gen_router_trait_method(
        &self,
        sub_service_ident: &Ident,
        sub_service_path: &Path,
    ) -> TokenStream2 {
        let method_ident = ident_to_case(sub_service_ident, Case::Snake);
        quote! {
            type #sub_service_ident: #sub_service_path;
            fn #method_ident(&self) -> &Self::#sub_service_ident;
        }
    }

    fn gen_router_trait(&self) -> TokenStream2 {
        let router_trait_ident = self.router_trait_ident();
        let items = self.items.iter().map(
            |RouterItem {
                 attrs: _,
                 ident,
                 paren_token: _,
                 path,
             }| self.gen_router_trait_method(ident, path.first().unwrap()),
        );

        quote! {
            trait #router_trait_ident {
                #( #items )*
            }
        }
    }

    fn gen_router_impl_trait_dispatcher(&self) -> TokenStream2 {
        let (router_trait_ident, service_request_ident, service_response_ident) = (
            self.router_trait_ident(),
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
        );

        let (trait_where_items, match_items): (Vec<_>, Vec<_>) = self
            .items
            .iter()
            .map(
                |RouterItem {
                     attrs: _,
                     ident,
                     paren_token: _,
                     path,
                 }| {
                    let sub_service_path = path.first().unwrap();
                    let method_ident = ident_to_case(ident, Case::Snake);

                    (
                        quote! {
                            Router::#ident: #sub_service_path
                        },
                        quote! {
                            #service_request_ident::#ident(req) => req
                                .dispatch(service.#method_ident())
                                .await
                                .map(|resp| #service_response_ident::#ident(resp)),
                        },
                    )
                },
            )
            .unzip();

        quote! {
            impl<Router> Dispatcher<Router, #service_response_ident> for #service_request_ident
            where
                Router: #router_trait_ident,
                #( #trait_where_items ),*
            {
                type Result<'a> = impl Future<Output = mrpc::AnyResult<#service_response_ident>>
                where
                    Router: 'a;

                fn dispatch<'a>(self, service: &'a Router) -> Self::Result<'a>
                where
                    Router: 'a
                {
                    async move {
                        match self {
                            #( #match_items )*
                        }
                    }
                }
            }
        }
    }

    fn router_sender_trait_ident(&self) -> Ident {
        format_ident!("{}Sender", self.ident)
    }

    fn gen_router_sender_trait_and_impl(&self) -> TokenStream2 {
        let (router_sender_trait_ident, service_request_ident, service_response_ident) = (
            self.router_sender_trait_ident(),
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
        );

        let (trait_method_items, impl_method_items): (Vec<_>, Vec<_>) = self
            .items
            .iter()
            .map(
                |RouterItem {
                     attrs: _,
                     ident,
                     paren_token: _,
                     path,
                 }| {
                    let sub_service_path = path.first().unwrap();
                    let sub_service_request_path = common::service_request_path(sub_service_path);
                    let sub_service_response_path = common::service_response_path(sub_service_path);

                    let method_ident = ident_to_case(ident, Case::Snake);

                    (
                        quote! {
                            fn #method_ident(&self) ->
                                mrpc::RouterSender<
                                        Sender,
                                        #service_request_ident,
                                        #service_response_ident,
                                        #sub_service_request_path,
                                        #sub_service_response_path>;
                        },
                        quote! {
                                fn #method_ident(&self) ->
                                mrpc::RouterSender<
                                        Sender,
                                        #service_request_ident,
                                        #service_response_ident,
                                        #sub_service_request_path,
                                        #sub_service_response_path> {
                                    mrpc::RouterSender::new(self)
                                }
                        },
                    )
                },
            )
            .unzip();

        quote! {
            trait #router_sender_trait_ident<Sender> {
                #( #trait_method_items )*
            }

            impl<Sender> #router_sender_trait_ident<Sender> for Sender
            where
                Sender: mrpc::Sender<Request = #service_request_ident, Response = #service_response_ident>,
            {
                #( #impl_method_items )*
            }
        }
    }
}

impl Parse for Router {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            enum_token: input.parse()?,
            ident: input.parse()?,
            brace_token: braced!(content in input),
            items: content.parse_terminated(RouterItem::parse)?,
        })
    }
}

impl ToTokens for Router {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_router_request_response(),
            self.gen_convert(),
            self.gen_router_trait(),
            self.gen_router_impl_trait_dispatcher(),
            self.gen_router_sender_trait_and_impl(),
        ])
    }
}

pub fn parse(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    parse_macro_input!(input as Router)
        .into_token_stream()
        .into()
}
