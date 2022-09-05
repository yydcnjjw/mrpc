mod rpc_method;

use convert_case::Case;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Attribute, Token, Visibility,
};

use crate::common::{self, ident_to_case};

use self::rpc_method::{RpcMethod, RpcSignature};

pub struct Service {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub trait_token: Token![trait],
    pub ident: Ident,
    pub brace_token: token::Brace,
    pub items: Punctuated<RpcMethod, Token![;]>,
}

impl Service {
    fn gen_rpc_request(&self, rpc_method: &RpcMethod) -> TokenStream2 {
        let (rpc_request_ident, args, attrs) = (
            common::rpc_request_ident(&rpc_method.sig.ident),
            rpc_method.sig.args.iter(),
            &rpc_method.attrs,
        );

        quote! {
            #( #attrs )*
            #rpc_request_ident{ #( #args ),* }
        }
    }

    fn gen_rpc_response(&self, rpc_method: &RpcMethod) -> TokenStream2 {
        let (rpc_response_ident, return_type, attrs) = (
            common::rpc_response_ident(&rpc_method.sig.ident),
            &rpc_method.sig.output,
            &rpc_method.attrs,
        );

        quote! {
            #( #attrs )*
            #rpc_response_ident( #return_type )
        }
    }

    fn gen_service_request_response(&self) -> TokenStream2 {
        let (attrs, vis, service_request_ident, service_response_ident) = (
            &self.attrs,
            &self.vis,
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
        );

        let (request_items, response_items): (Vec<_>, Vec<_>) = self
            .items
            .iter()
            .map(|rpc_method| {
                (
                    self.gen_rpc_request(rpc_method),
                    self.gen_rpc_response(rpc_method),
                )
            })
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

    fn gen_service_trait_method(&self, rpc_method: &RpcMethod) -> TokenStream2 {
        let RpcSignature {
            fn_token: _,
            ident,
            paren_token: _,
            args,
            output,
        } = &rpc_method.sig;

        let args = args.iter();

        let output_assoc_type = ident_to_case(&format_ident!("{}Output", &ident), Case::UpperCamel);

        let async_future_lifetime = quote!('__async_future);

        quote! {
            type #output_assoc_type<#async_future_lifetime>: std::future::Future<Output = mrpc::AnyResult<#output>>
            where
                Self: #async_future_lifetime;

            fn #ident(&self, #( #args ),*) -> Self::#output_assoc_type<'_>;
        }
    }

    fn gen_service_trait(&self) -> TokenStream2 {
        let (vis, ident) = (&self.vis, &self.ident);

        let trait_methods = self
            .items
            .iter()
            .map(|rpc_method| self.gen_service_trait_method(rpc_method));

        quote! {
            #vis trait #ident {
                #( #trait_methods )*
            }
        }
    }

    fn gen_service_impl_dispatcher(&self) -> TokenStream2 {
        let (service_ident, service_request_ident, service_response_ident) = (
            &self.ident,
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
        );

        let rpcs = self.items.iter().map(|rpc_method| {
            let ident = &rpc_method.sig.ident;
            let arg_pats = rpc_method
                .sig
                .args
                .iter()
                .map(|arg| &*arg.pat)
                .collect::<Vec<_>>();

            let rpc_request_ident = common::rpc_request_ident(&ident);
            let rpc_response_ident = common::rpc_response_ident(&ident);
            quote! {
                #service_request_ident::#rpc_request_ident{#( #arg_pats ),*}
                => #service_ident::#ident(service, #( #arg_pats ),*).await
                .map(|v| #service_response_ident::#rpc_response_ident(v))
            }
        });

        let async_future_lifetime = quote!('__async_future);

        quote! {
            impl<Service> mrpc::Dispatcher<Service, #service_response_ident> for #service_request_ident
            where
                Service: #service_ident,
            {
                type Result<#async_future_lifetime> = impl std::future::Future<Output = mrpc::AnyResult<#service_response_ident>>
                where Service: #async_future_lifetime;
                
                fn dispatch<#async_future_lifetime>(
                    self,
                    service: &#async_future_lifetime Service
                ) -> Self::Result<#async_future_lifetime>
                where
                    Service: #async_future_lifetime
                {
                    async move {
                        match self {
                            #( #rpcs ),*
                        }
                    }
                }
            }
        }
    }

    fn gen_service_impl_trait_method_with_sender(&self, rpc_method: &RpcMethod) -> TokenStream2 {
        let RpcSignature {
            fn_token: _,
            ident,
            paren_token: _,
            args,
            output,
        } = &rpc_method.sig;

        let args = args.iter();

        let service_request_ident = common::service_request_ident(&self.ident);
        let service_response_ident = common::service_response_ident(&self.ident);
        let rpc_request_ident = common::rpc_request_ident(&ident);
        let rpc_response_ident = common::rpc_response_ident(&ident);

        let output_assoc_type = ident_to_case(&format_ident!("{}Output", &ident), Case::UpperCamel);

        let async_future_lifetime = quote!('__async_future);

        quote! {
            type #output_assoc_type<#async_future_lifetime> = impl std::future::Future<Output = mrpc::AnyResult<#output>>
            where
                Self: #async_future_lifetime;

            fn #ident(&self, #( #args ),*) -> Self::#output_assoc_type<'_> {
                async move {
                    match self.send(#service_request_ident::#rpc_request_ident { a, b }).await? {
                        #service_response_ident::#rpc_response_ident(v) => Ok(v),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn gen_service_impl_trait_with_sender(&self) -> TokenStream2 {
        let (ident, service_request_ident, service_response_ident) = (
            &self.ident,
            common::service_request_ident(&self.ident),
            common::service_response_ident(&self.ident),
        );

        let impl_methods = self
            .items
            .iter()
            .map(|rpc_method| self.gen_service_impl_trait_method_with_sender(rpc_method));

        quote! {
            impl<T> #ident for T
            where
                T: mrpc::Sender<Request = #service_request_ident, Response = #service_response_ident>,
            {
                #( #impl_methods )*
            }
        }
    }
}

impl Parse for Service {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            trait_token: input.parse()?,
            ident: input.parse()?,
            brace_token: braced!(content in input),
            items: content.parse_terminated(RpcMethod::parse)?,
        })
    }
}

impl ToTokens for Service {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_service_request_response(),
            self.gen_service_trait(),
            self.gen_service_impl_dispatcher(),
            self.gen_service_impl_trait_with_sender(),
        ])
    }
}

pub fn parse(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    parse_macro_input!(input as Service)
        .into_token_stream()
        .into()
}
