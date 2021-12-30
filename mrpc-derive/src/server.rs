use crate::common::*;
use convert_case::Case;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Attribute, Ident, Path, Token, Visibility,
};

#[allow(dead_code)]
struct ServiceItem {
    pub attrs: Vec<Attribute>,
    pub ident: Ident,
    pub paren_token: token::Paren,
    pub ty: Path,
}

impl Parse for ServiceItem {
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

#[allow(dead_code)]
struct Server {
    pub vis: Visibility,
    pub enum_token: Token![enum],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub services: Punctuated<ServiceItem, Token![,]>,
}

impl Parse for Server {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            vis: input.parse()?,
            enum_token: input.parse()?,
            ident: input.parse()?,
            brace_token: braced!(content in input),
            services: content.parse_terminated(ServiceItem::parse)?,
        })
    }
}

impl Server {
    fn request_ident(&self) -> Ident {
        format_ident!("{}Request", self.ident)
    }

    fn response_ident(&self) -> Ident {
        format_ident!("{}Response", self.ident)
    }

    fn server_ident(&self) -> Ident {
        self.ident.clone()
    }

    fn client_ident(&self) -> Ident {
        format_ident!("{}Client", self.ident)
    }

    fn get_service_ident(ident: &Ident) -> Ident {
        format_ident!("get_{}", ident_to_case(ident, Case::Snake))
    }

    fn service_request_ty(mut ty: Path) -> Path {
        let mut last = ty.segments.last_mut().expect("");
        last.ident = format_ident!("{}Request", last.ident);
        ty
    }

    fn service_response_ty(mut ty: Path) -> Path {
        let mut last = ty.segments.last_mut().expect("");
        last.ident = format_ident!("{}Response", last.ident);
        ty
    }

    fn service_client_ty(mut ty: Path) -> Path {
        let mut last = ty.segments.last_mut().expect("");
        last.ident = format_ident!("{}Client", last.ident);
        ty
    }

    fn gen_server(&self) -> TokenStream2 {
        let (vis, server_ident, request_ident) =
            (&self.vis, self.server_ident(), self.request_ident());

        let fn_get_services = self
            .services
            .iter()
            .map(
                |ServiceItem {
                     attrs: _,
                     ident,
                     paren_token: _,
                     ty,
                 }| {
                    let get_service_ident =
                        Self::get_service_ident(ident);

                    quote! {
                        async fn #get_service_ident(self: Arc<Self>) -> mrpc::anyhow::Result<std::sync::Arc<dyn #ty>> {
                            mrpc::anyhow::bail!("service is not implemented");
                        }
                    }
                },
            )
            .collect::<Vec<_>>();

        let fn_serve = {
            let response_ident = self.response_ident();
            let match_items = self
                .services
                .iter()
                .map(
                    |ServiceItem {
                         attrs: _,
                         ident,
                         paren_token: _,
                         ty: _,
                     }| {
                        let get_service_ident = Self::get_service_ident(ident);
                        quote! {
                             #request_ident::#ident(req) => {
                                match Self::#get_service_ident(self.clone()).await {
                                    Ok(service) => {
                                        Ok(#response_ident::#ident(service.serve(req).await))
                                    }
                                    Err(e) => {
                                        Err(mrpc::anyhow::anyhow!("{}", e))
                                    }
                                }
                            }
                        }
                    },
                )
                .collect::<Vec<_>>();

            quote! {
                async fn serve(self: Arc<Self>,
                               mut rx: mrpc::tokio::sync::mpsc::Receiver<mrpc::Message<#request_ident, #response_ident>>) {
                    while let Some(msg) = rx.recv().await {
                        let resp = match msg.req {
                            #( #match_items )*
                        };

                        match resp {
                            Ok(resp) => {
                                if let Err(_) = msg.resp.send(resp) {
                                    println!("Send failed");
                                }
                            }
                            Err(e) => {
                                println!("{}", e);
                            }
                        }


                    }
                }
            }
        };

        quote! {
            #[mrpc::async_trait]
            #vis trait #server_ident: Send + Sync {
                #( #fn_get_services )*
                #fn_serve
            }
        }
    }

    fn gen_request(&self) -> TokenStream2 {
        let (vis, request_ident) = (&self.vis, self.request_ident());

        let services = self.services.iter().map(
            |ServiceItem {
                 attrs,
                 ident,
                 paren_token: _,
                 ty,
             }| {
                let service_request = Self::service_request_ty(ty.clone());

                quote! {
                    #( #attrs )*
                    #ident( #service_request )
                }
            },
        );

        quote! {
            // #[derive(Debug, mrpc::serde::Serialize, mrpc::serde::Deserialize)]
            // #[serde(crate = "mrpc::serde")]
            #vis enum #request_ident {
                #( #services ),*
            }
        }
    }

    fn gen_response(&self) -> TokenStream2 {
        let (vis, response_ident) = (&self.vis, self.response_ident());

        let services = self.services.iter().map(
            |ServiceItem {
                 attrs,
                 ident,
                 paren_token: _,
                 ty,
             }| {
                let service_response = Self::service_response_ty(ty.clone());

                quote! {
                    #( #attrs )*
                    #ident( #service_response )
                }
            },
        );

        quote! {
            #[derive(Debug)]
            // #[derive(Debug, mrpc::serde::Serialize, mrpc::serde::Deserialize)]
            // #[serde(crate = "mrpc::serde")]
            #vis enum #response_ident {
                #( #services ),*
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

        let sender_ty = quote! {
            mrpc::tokio::sync::mpsc::Sender<mrpc::Message<#request_ident, #response_ident>>
        };

        let (posters, rpcs): (Vec<TokenStream2>, Vec<TokenStream2>) = self.services.iter().map(
            |ServiceItem {
                 attrs: _,
                 ident,
                 paren_token: _,
                 ty,
             }| {
                let service_ident = ident_to_case(ident, Case::Snake);
                let service_client = Self::service_client_ty(ty.clone());
                let service_request = Self::service_request_ty(ty.clone());
                let service_response = Self::service_response_ty(ty.clone());
                let service_poster_ident = format_ident!("{}Poster", ident);

                (
                    quote! {
                        #vis struct #service_poster_ident {
                            pub sender: #sender_ty
                        }

                        #[mrpc::async_trait]
                        impl mrpc::Poster<#service_request, #service_response> for #service_poster_ident {
                            async fn post(&self, req: #service_request,
                                          resp: mrpc::tokio::sync::oneshot::Sender<
                                                  #service_response
                                              >) -> mrpc::anyhow::Result<()> {
                                let (tx, rx) = mrpc::tokio::sync::oneshot::channel();

                                if let Err(e) = self.sender.send(mrpc::Message {
                                    req: #request_ident::#ident(req),
                                    resp: tx
                                }).await {
                                    mrpc::anyhow::bail!("Send message failed: {}", e);
                                }

                                match rx.await {
                                    Ok(recv) => {
                                        match recv {
                                            #response_ident::#ident(v) => {
                                                if let Err(e) = resp.send(v) {
                                                    mrpc::anyhow::bail!("Send message to {} failed", stringify!(#ident));    
                                                } else {
                                                    return Ok(())
                                                }
                                            }
                                            _ => {
                                                mrpc::anyhow::bail!("Response not match require {}", stringify!(#ident));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        mrpc::anyhow::bail!("Wait response failed: {}", e);
                                    }
                                }
                            }
                        }
                    },
                    quote! {
                        #vis fn #service_ident(&self) -> #service_client<#service_poster_ident> {
                            #service_client { poster: #service_poster_ident { sender: self.sender.clone() } }
                        }
                    },
                )
            },
        ).unzip();

        quote! {
            #vis struct #client_ident {
                sender: #sender_ty,
            }

            #( #posters )*

            impl #client_ident {
                #( #rpcs )*
            }
        }
    }
}

impl ToTokens for Server {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_request(),
            self.gen_response(),
            self.gen_server(),
            self.gen_client(),
        ])
    }
}

pub fn parse(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let service = parse_macro_input!(input as Server);
    service.into_token_stream().into()
}
