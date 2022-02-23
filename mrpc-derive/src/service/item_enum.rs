use crate::{common::*, service::ident::{service_request_ty, service_response_ty, service_api_ty}};
use convert_case::Case;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token, Attribute, Ident, Path, Token, Visibility,
};

use super::attr::Attrs as ServiceAttrs;

#[allow(dead_code)]
pub struct EnumItem {
    pub attrs: Vec<Attribute>,
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub ty: Path,
}

impl Parse for EnumItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            ident: input.parse()?,
            paren_token: parenthesized!(content in input),
            ty: content.parse::<Path>()?,
        })
    }
}

pub struct ItemEnum {
    pub attrs: ServiceAttrs,
    pub vis: Visibility,
    pub enum_token: Token![enum],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub services: Punctuated<EnumItem, Token![,]>,
}

impl Parse for ItemEnum {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attrs: ServiceAttrs::new(),
            vis: input.parse()?,
            enum_token: input.parse()?,
            ident: input.parse()?,
            brace_token: braced!(content in input),
            services: {
                let result = content.parse_terminated(EnumItem::parse)?;

                if result.is_empty() {
                    return Err(syn::Error::new(
                        content.span(),
                        "At least one service is required",
                    ));
                }

                result
            },
        })
    }
}

impl ItemEnum {
    fn request_ident(&self) -> Ident {
        format_ident!("{}Request", self.ident)
    }

    fn response_ident(&self) -> Ident {
        format_ident!("{}Response", self.ident)
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

    fn create_service_ident(ident: &Ident) -> Ident {
        format_ident!("create_{}", ident_to_case(ident, Case::Snake))
    }

    fn service_var_ident(ident: &Ident) -> Ident {
        format_ident!("{}_var", ident_to_case(ident, Case::Snake))
    }

    fn gen_service_create_services(&self) -> Vec<TokenStream2> {
        self
            .services
            .iter()
            .map(
                |EnumItem {
                    attrs: _,
                    ident,
                    paren_token: _,
                    ty,
                }| {
                    let create_service_ident =
                        Self::create_service_ident(ident);
                    let service_request_ty = service_request_ty(ty.clone());
                    let service_response_ty = service_response_ty(ty.clone());

                    quote! {
                        async fn #create_service_ident(
                            self: mrpc::sync::Arc<Self>)
                            -> mrpc::anyhow::Result<mrpc::SharedService<
                                    #service_request_ty, #service_response_ty
                                >>
                        where Self: Send + Sync + 'static {
                            mrpc::anyhow::bail!("service is not implemented");
                        }
                    }
                },
            )
            .collect::<Vec<_>>()
    }

    fn gen_service_serve(&self) -> TokenStream2 {
        let (request_ident, response_ident) = (self.request_ident(), self.response_ident());

        let (static_services, match_items): (Vec<_>, Vec<_>) = self
            .services
            .iter()
            .map(
                |EnumItem {
                    attrs: _,
                    ident,
                    paren_token: _,
                    ty,
                }| {
                    let create_service_ident = Self::create_service_ident(ident);
                    let static_service_ident = Self::service_var_ident(ident);
                    let service_request_ty = service_request_ty(ty.clone());
                    let service_response_ty = service_response_ty(ty.clone());
                    
                    (
                        quote! {
                            static #static_service_ident: mrpc::sync::OnceCell<mrpc::SharedService<
                                    #service_request_ty, #service_response_ty
                                >> =
                                mrpc::sync::OnceCell::const_new();
                        },
                        quote! {
                            #request_ident::#ident(service_request) => {
                                let service = 
                                    match #static_service_ident.get_or_try_init(|| async move {
                                        Self::#create_service_ident(self).await
                                    }).await {
                                        Ok(v) => v.clone(),
                                        Err(e) => {
                                            mrpc::anyhow::bail!("Failed to initialize {}: {:?}", stringify!(#ident), e);
                                        }
                                    };

                                Ok(#response_ident::#ident(service.serve(service_request).await?))
                            }
                        },
                    )
                },
            )
            .unzip();

        quote! {
            async fn serve(self: mrpc::sync::Arc<Self>,
                           request: #request_ident)
                           -> mrpc::anyhow::Result<#response_ident>
            where Self: Send + Sync + 'static {

                #( #static_services )*

                match request {
                    #( #match_items )*
                }
            }
        }
    }

    fn gen_service(&self) -> TokenStream2 {
        let (vis,
             service_ident,
             shared_service_ident,
             request_ident,
             response_ident
        ) = (&self.vis, self.service_ident(), self.shared_service_ident(), self.request_ident(), self.response_ident());

        let fn_create_services = self.gen_service_create_services();
        let fn_serve = self.gen_service_serve();

        quote! {
            #[mrpc::async_trait]
            #vis trait #service_ident {
                #( #fn_create_services )*
                
                #fn_serve
            }

            #vis type #shared_service_ident = mrpc::SharedService<#request_ident, #response_ident>;
        }
    }

    fn gen_request(&self) -> TokenStream2 {
        let (message_attr, vis, request_ident) = (self.attrs.gen_message_attr(), &self.vis, self.request_ident());

        let services = self.services.iter().map(
            |EnumItem {
                attrs,
                ident,
                paren_token: _,
                ty,
            }| {
                let service_request = service_request_ty(ty.clone());

                quote! {
                    #( #attrs )*
                    #ident( #service_request )
                }
            },
        );

        quote! {
            #message_attr
            #vis enum #request_ident {
                #( #services ),*
            }
        }
    }

    fn gen_response(&self) -> TokenStream2 {
        let (message_attr, vis, response_ident) = (self.attrs.gen_message_attr(), &self.vis, self.response_ident());

        let services = self.services.iter().map(
            |EnumItem {
                attrs,
                ident,
                paren_token: _,
                ty,
            }| {
                let service_response = service_response_ty(ty.clone());

                quote! {
                    #( #attrs )*
                    #ident( #service_response )
                }
            },
        );

        quote! {
            #message_attr
            #vis enum #response_ident {
                #( #services ),*
            }
        }
    }

    fn gen_api(&self) -> TokenStream2 {
        let (vis, api_ident, request_ident, response_ident, client_ident) = (
            &self.vis,
            self.api_ident(),
            self.request_ident(),
            self.response_ident(),
            self.client_ident()
        );

        let (rpcs, service_senders): (Vec<TokenStream2>, Vec<TokenStream2>) = self.services.iter().map(
            |EnumItem {
                attrs: _,
                ident,
                paren_token: _,
                ty,
            }| {
                let service_ident = ident_to_case(ident, Case::Snake);
                let service_api_ty = service_api_ty(ty.clone());
                let service_request_ty = service_request_ty(ty.clone());
                let service_response_ty = service_response_ty(ty.clone());

                let service_sender_ident = format_ident!("{}Sender", ident);

                (
                    quote! {                    
                        #vis fn #service_ident(&self) -> mrpc::Client<#service_api_ty> {
                            mrpc::Client::new(mrpc::sync::Arc::new(#service_sender_ident {
                                sender: self.sender.clone()
                            }))
                        }
                    },
                    quote! {
                        struct #service_sender_ident {
                            sender: mrpc::client::Sender<#request_ident, #response_ident>,
                        }
                        
                        #[mrpc::async_trait]
                        impl mrpc::Sender<#service_request_ty, #service_response_ty> for #service_sender_ident {
                            async fn send_request(&self, request: #service_request_ty) -> mrpc::anyhow::Result<#service_response_ty> {
                                let response = self.sender.send_request(#request_ident::#ident(request)).await;

                                response.and_then(|v| match v {
                                    #response_ident::#ident(o) => {
                                        Ok(o)
                                    }
                                    _ => {
                                        Err(mrpc::anyhow::anyhow!(
                                            "Failed to get response: not match require {}",
                                            stringify!(#ident)))
                                    }
                                })
                            }
                        }
                    }
                ) 
            },
        ).unzip();

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
            
            #( #service_senders )*
        }
    }
}

impl ToTokens for ItemEnum {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_request(),
            self.gen_response(),
            self.gen_service(),
            self.gen_api(),
        ])
    }
}
