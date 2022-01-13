use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{parse::Parse, Path};

use crate::service::ident::{service_request_ty, service_response_ty};

pub struct ItemImpl {
    inner: syn::ItemImpl,
}

impl Parse for ItemImpl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let inner: syn::ItemImpl = input.parse()?;

        if inner.trait_.is_none() {
            return Err(syn::Error::new(input.span(), "Only Support impl trait"));
        }

        Ok(Self { inner })
    }
}

impl ItemImpl {
    fn trait_service_ty(&self) -> Path {
        let (_, ty, _) = self.inner.trait_.as_ref().unwrap();
        ty.clone()
    }

    fn gen_impl_service(&self) -> TokenStream2 {
        let trait_service_ty = self.trait_service_ty();
        let (impl_service_ty, service_request_ty, service_response_ty) = (
            self.inner.self_ty.as_ref(),
            service_request_ty(trait_service_ty.clone()),
            service_response_ty(trait_service_ty.clone()),
        );

        quote! {
            #[mrpc::async_trait]
            impl mrpc::Service for #impl_service_ty {
                type Request = #service_request_ty;
                type Response = #service_response_ty;
                async fn serve(self: Arc<Self>, request: Self::Request)
                               -> mrpc::anyhow::Result<Self::Response> {
                    <#impl_service_ty as #trait_service_ty>::serve(self, request).await
                }
            }
        }
    }

    fn gen_impl_trait(&self) -> TokenStream2 {
        let orgin = self.inner.to_token_stream();
        quote! {
            #[mrpc::async_trait]
            #orgin
        }
    }
}

impl ToTokens for ItemImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([
            self.gen_impl_trait(),
            self.gen_impl_service()
        ])
    }
}
