use proc_macro::TokenStream;
use std::sync::Mutex;
use quote::{format_ident, quote};
use syn::{braced, parse_macro_input, Field, Ident, Token, Visibility, Type};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Brace;

mod keywords {
    syn::custom_keyword!(initiator);
    syn::custom_keyword!(actor);
    syn::custom_keyword!(sensor);
}

enum ModuleAttrs {
    Nothing,
    Initiator,
    Actor,
    Sensor,
}

impl Parse for ModuleAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(ModuleAttrs::Nothing)
        } else {
            let lookahead = input.lookahead1();
            if lookahead.peek(keywords::initiator) {
                Ok(ModuleAttrs::Initiator)
            } else if lookahead.peek(keywords::actor) {
                Ok(ModuleAttrs::Actor)
            } else if lookahead.peek(keywords::sensor) {
                Ok(ModuleAttrs::Sensor)
            } else {
                Err(input.error("Module type must be empty or one of \"initiator\", \"actor\", or \
                \"sensor\""))
            }
        }
    }
}

struct ModuleInput {
    pub ident: Ident,
    pub fields: Punctuated<Field, Token![,]>,
}

impl Parse for ModuleInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![pub]) {
            let _vis: Visibility = input.parse()?;
        }
        if input.parse::<Token![struct]>().is_err() {
            return Err(input.error("Modules must be structs"));
        }
        let ident = input.parse::<Ident>()?;

        let lookahead = input.lookahead1();
        if !lookahead.peek(Brace) {
            return Err(input.error("Modules can't be unit structs"));
        }

        let content;
        braced!(content in input);
        Ok(Self {
            ident,
            fields: content.parse_terminated(Field::parse_named)?,
        })
    }
}

#[proc_macro_attribute]
pub fn module(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr as ModuleAttrs);
    let item = parse_macro_input!(tokens as ModuleInput);

    let output = {
        let ident = item.ident;
        let fields = item.fields.iter();
        quote! {
            pub struct #ident {
                #(#fields),*
            }
        }
    };
    output.into()
}