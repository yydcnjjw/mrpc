use convert_case::Case;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, ImplItem, ItemImpl, ReturnType, Type,
};

use crate::common::ident_to_case;

pub struct ServiceImpl {
    inner: ItemImpl,
}

impl ServiceImpl {
    fn gen_service_impl(&self) -> TokenStream2 {
        let mut item_impl = self.inner.clone();

        let items = item_impl.items.iter_mut().filter_map(|item| match item {
            ImplItem::Method(v) => Some(v),
            _ => None,
        }).map(|method| {
            let output_ty: Type = match &method.sig.output {
                ReturnType::Default => parse_quote!(()),
                ReturnType::Type(_, ty)=> *ty.clone(),
            };

            let method_ident = &method.sig.ident;

            let output_assoc_type_ident = format_ident!("{}Output", ident_to_case(&method_ident, Case::UpperCamel));

            method.sig.asyncness = None;
            method.sig.output = parse_quote!(
                -> Self::#output_assoc_type_ident<'_>
            );
            
            let stmts: Vec<_> = method.block.stmts.drain(..).collect();

            method.block.stmts.push(parse_quote!(async move {
                #( #stmts )*
            }));

            parse_quote! {
                type #output_assoc_type_ident<'a> = impl std::future::Future<Output = #output_ty>;
            }

        }).collect::<Vec<ImplItem>>();

        item_impl.items.extend(items);

        quote! {
            #item_impl
        }
    }
}

impl Parse for ServiceImpl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner: ItemImpl = input.parse()?;

        if inner.trait_.is_none() {
            return Err(syn::Error::new(input.span(), "Only Support impl trait"));
        }

        Ok(Self { inner })
    }
}

impl ToTokens for ServiceImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        tokens.extend([self.gen_service_impl()])
    }
}

pub fn parse(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    parse_macro_input!(input as ServiceImpl)
        .into_token_stream()
        .into()
}
