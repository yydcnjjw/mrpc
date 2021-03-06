use crate::{common::*, attr::{MessageAttr, set_only_none}};
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

struct ServerAttrs {
    message: Option<MessageAttr>,
}

impl ServerAttrs {
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

impl Parse for ServerAttrs {
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
struct Server {
    pub server_attrs: ServerAttrs,
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
            server_attrs: ServerAttrs::new(),
            vis: input.parse()?,
            enum_token: input.parse()?,
            ident: input.parse()?,
            brace_token: braced!(content in input),
            services: {
                let result = content.parse_terminated(ServiceItem::parse)?;

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

    fn create_service_ident(ident: &Ident) -> Ident {
        format_ident!("create_{}", ident_to_case(ident, Case::Snake))
    }

    fn service_var_ident(ident: &Ident) -> Ident {
        format_ident!("{}_var", ident_to_case(ident, Case::Snake))
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

    fn service_poster_ty(mut ty: Path) -> Path {
        let mut last = ty.segments.last_mut().expect("");
        last.ident = format_ident!("{}Poster", last.ident);
        ty
    }

    fn gen_server_create_services(&self) -> Vec<TokenStream2> {
        self
            .services
            .iter()
            .map(
                |ServiceItem {
                     attrs: _,
                     ident,
                     paren_token: _,
                     ty,
                 }| {
                    let create_service_ident =
                        Self::create_service_ident(ident);

                    quote! {
                        async fn #create_service_ident(self: std::sync::Arc<Self>) -> mrpc::anyhow::Result<std::sync::Arc<dyn #ty>> {
                            mrpc::anyhow::bail!("service is not implemented");
                        }
                    }
                },
            )
            .collect::<Vec<_>>()
    }

    fn gen_server_serve(&self) -> TokenStream2 {
        let (request_ident, response_ident) = (self.request_ident(), self.response_ident());

        let (service_vars, match_items): (Vec<_>, Vec<_>) = self
            .services
            .iter()
            .map(
                |ServiceItem {
                     attrs: _,
                     ident,
                     paren_token: _,
                     ty: _,
                 }| {
                    let create_service_ident = Self::create_service_ident(ident);
                    let service_var_ident = Self::service_var_ident(ident);

                    let service_var_ident_tmp = format_ident!("{}_tmp", service_var_ident);
                    
                    (
                        quote! {
                            let mut #service_var_ident = std::sync::Arc::new(mrpc::sync::Mutex::new(None));
                        },
                        quote! {
                            #request_ident::#ident(req) => {
                                let #service_var_ident_tmp = #service_var_ident.clone();
                                let self_ = self.clone();
                                mrpc::spawn(async move {
                                    let service = {
                                        let mut lock = #service_var_ident_tmp.lock().await;
                                        if lock.is_none() {
                                            *lock = match Self::#create_service_ident(self_).await {
                                                Ok(service) => Some(service),
                                                Err(e) => {
                                                    mrpc::log::warn!("Failed to create {}: {:?}", stringify!(#ident), e);
                                                    None
                                                }
                                            };
                                        }
                                        lock.as_ref().unwrap().clone()
                                    };

                                    let result = #response_ident::#ident(service.clone().serve(req).await);
                                    if let Err(_) = msg.resp.send(result) {
                                        mrpc::log::warn!("Failed to send response: {}", stringify!(#ident));
                                    }
                                });
                            }
                        },
                    )
                },
            )
            .unzip();

        quote! {
            async fn serve(self: std::sync::Arc<Self>,
                           mut rx: mrpc::sync::mpsc::Receiver<mrpc::Message<#request_ident, #response_ident>>)
                           -> mrpc::anyhow::Result<()>
            where Self: 'static {

                #( #service_vars )*

                while let Some(msg) = rx.recv().await {
                    match msg.req {
                        #( #match_items )*     
                    };
                }

                Ok(())
            }
        }
    }

    fn gen_server(&self) -> TokenStream2 {
        let (vis, server_ident) = (&self.vis, self.server_ident());

        let fn_create_services = self.gen_server_create_services();
        let fn_serve = self.gen_server_serve();

        quote! {
            #[mrpc::async_trait]
            #vis trait #server_ident: Send + Sync {
                #( #fn_create_services )*
                #fn_serve
            }
        }
    }

    fn gen_request(&self) -> TokenStream2 {
        let (message_attr, vis, request_ident) = (self.server_attrs.gen_message_attr(), &self.vis, self.request_ident());

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
            #message_attr
            #vis enum #request_ident {
                #( #services ),*
            }
        }
    }

    fn gen_response(&self) -> TokenStream2 {
        let (message_attr, vis, response_ident) = (self.server_attrs.gen_message_attr(), &self.vis, self.response_ident());

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
            #message_attr
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
            mrpc::sync::mpsc::Sender<mrpc::Message<#request_ident, #response_ident>>
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
                let service_poster_ty = Self::service_poster_ty(ty.clone());
                let service_poster_impl_ident = format_ident!("{}{}PosterImpl", self.server_ident(), ident);

                (
                    quote! {
                        #[derive(Clone)]
                        #vis struct #service_poster_impl_ident {
                            pub sender: #sender_ty
                        }

                        impl #service_poster_ty for #service_poster_impl_ident {}

                        #[mrpc::async_trait]
                        impl mrpc::Poster<#service_request, #service_response> for #service_poster_impl_ident {
                            async fn post(&self, req: #service_request,
                                          resp: mrpc::sync::oneshot::Sender<
                                                  #service_response
                                              >) -> mrpc::anyhow::Result<()> {
                                let (tx, rx) = mrpc::sync::oneshot::channel();

                                if let Err(e) = self.sender.send(mrpc::Message {
                                    req: #request_ident::#ident(req),
                                    resp: tx
                                }).await {
                                    mrpc::anyhow::bail!("Failed to send message: {}", e);
                                }

                                match rx.await {
                                    Ok(recv) => {
                                        match recv {
                                            #response_ident::#ident(v) => {
                                                if let Err(e) = resp.send(v) {
                                                    mrpc::anyhow::bail!("Failed to send message to {}", stringify!(#ident));    
                                                } else {
                                                    return Ok(())
                                                }
                                            }
                                            _ => {
                                                mrpc::anyhow::bail!("Failed to match response, require {}", stringify!(#ident));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        mrpc::anyhow::bail!("Failed to wait response: {}", e);
                                    }
                                }
                            }
                        }
                    },
                    quote! {
                        #vis fn #service_ident(&self) -> #service_client<#service_poster_impl_ident> {
                            #service_client { poster: #service_poster_impl_ident { sender: self.sender.clone() } }
                        }
                    },
                )
            },
        ).unzip();

        quote! {
            #[derive(Clone)]
            #vis struct #client_ident {
                sender: #sender_ty,
            }

            #( #posters )*

            impl #client_ident {

                #vis fn new(sender: #sender_ty) -> Self {
                    Self {
                        sender
                    }
                }
                
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

pub fn parse(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut server = parse_macro_input!(input as Server);
    server.server_attrs = parse_macro_input!(attrs as ServerAttrs);
    server.into_token_stream().into()
}
