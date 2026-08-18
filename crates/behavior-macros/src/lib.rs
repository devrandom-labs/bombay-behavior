//! `behavior-macros` — proc-macros for the behavior algebra.
//!
//! `#[actor]` wires the ordinary infallible/no-birth subset and `#[behavior]`
//! wires a fully declared inherent user-message fold.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    Error, FnArg, GenericArgument, ImplItem, ItemImpl, PathArguments, Result, ReturnType, Token,
    Type, parse_macro_input,
};

mod behavior_kw {
    syn::custom_keyword!(addr);
    syn::custom_keyword!(message);
    syn::custom_keyword!(sends);
    syn::custom_keyword!(births);
    syn::custom_keyword!(error);
}

fn crate_path(found: FoundCrate) -> TokenStream2 {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let name = syn::Ident::new(&name, Span::call_site());
            quote!(::#name)
        }
    }
}

fn behavior_crate() -> Result<TokenStream2> {
    if std::env::var("CARGO_PKG_NAME").as_deref() == Ok("bombay-behavior") {
        // This package deliberately exposes the library target as `behavior`,
        // not Cargo's normalized package name `bombay_behavior`. The same path
        // works in its unit, integration, and rustdoc crates.
        return Ok(quote!(::behavior));
    }
    if let Ok(found) = crate_name("bombay-behavior") {
        return Ok(crate_path(found));
    }
    if let Ok(found) = crate_name("bombay-rs") {
        let bombay = crate_path(found);
        return Ok(quote!(#bombay::behavior));
    }
    Err(Error::new(
        Span::call_site(),
        "could not resolve `bombay-behavior` directly or through `bombay-rs`",
    ))
}

struct BehaviorArgs {
    addr: Type,
    message: Type,
    sends: Type,
    births: Type,
    error: Type,
}

impl Parse for BehaviorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<behavior_kw::addr>()?;
        input.parse::<Token![=]>()?;
        let addr = input.parse()?;
        input.parse::<Token![,]>()?;
        input.parse::<behavior_kw::message>()?;
        input.parse::<Token![=]>()?;
        let message = input.parse()?;
        input.parse::<Token![,]>()?;
        input.parse::<behavior_kw::sends>()?;
        input.parse::<Token![=]>()?;
        let sends = input.parse()?;
        input.parse::<Token![,]>()?;
        input.parse::<behavior_kw::births>()?;
        input.parse::<Token![=]>()?;
        let births = input.parse()?;
        input.parse::<Token![,]>()?;
        input.parse::<behavior_kw::error>()?;
        input.parse::<Token![=]>()?;
        let error = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input
                .error("expected exactly addr, message, sends, births, and error in that order"));
        }
        Ok(Self {
            addr,
            message,
            sends,
            births,
            error,
        })
    }
}

fn validate_receiver(method: &syn::ImplItemFn) -> Result<()> {
    let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
        return Err(Error::new_spanned(
            &method.sig,
            "behavior methods must begin with &mut self",
        ));
    };
    if !matches!(receiver.kind, syn::ReceiverKind::Reference(_, _, Some(_))) {
        return Err(Error::new_spanned(
            receiver,
            "behavior methods must begin with &mut self",
        ));
    }
    if method.sig.constness.is_some()
        || method.sig.asyncness.is_some()
        || matches!(method.sig.safety, syn::Safety::Unsafe(_))
        || !method.sig.generics.params.is_empty()
    {
        return Err(Error::new_spanned(
            &method.sig,
            "behavior init and receive methods must be synchronous, safe, and non-generic",
        ));
    }
    if matches!(method.sig.output, ReturnType::Default) {
        return Err(Error::new_spanned(
            &method.sig,
            "behavior methods must declare their complete Actions result type",
        ));
    }
    Ok(())
}

/// Define the common synchronous, infallible, no-birth behavior subset.
#[proc_macro_attribute]
pub fn actor(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return Error::new(Span::call_site(), "#[actor] accepts no arguments")
            .to_compile_error()
            .into();
    }
    let item = parse_macro_input!(item as ItemImpl);
    if item.trait_.is_some() {
        return Error::new_spanned(&item, "#[actor] applies to an inherent impl")
            .to_compile_error()
            .into();
    }
    let Some(receive) = item.items.iter().find_map(|item| match item {
        ImplItem::Fn(method) if method.sig.ident == "receive" => Some(method),
        _ => None,
    }) else {
        return Error::new_spanned(
            &item.self_ty,
            "#[actor] requires receive(&mut self, from, message)",
        )
        .to_compile_error()
        .into();
    };
    if let Err(error) = validate_receiver(receive) {
        return error.to_compile_error().into();
    }
    if receive.sig.inputs.len() != 3 {
        return Error::new_spanned(
            &receive.sig,
            "receive must accept exactly &mut self, from, and message",
        )
        .to_compile_error()
        .into();
    }
    let mut parameters = receive.sig.inputs.iter().skip(1);
    let Some(FnArg::Typed(from)) = parameters.next() else {
        return Error::new_spanned(&receive.sig, "from requires an explicit address type")
            .to_compile_error()
            .into();
    };
    let Some(FnArg::Typed(message)) = parameters.next() else {
        return Error::new_spanned(&receive.sig, "message requires an explicit type")
            .to_compile_error()
            .into();
    };
    let ReturnType::Type(_, result) = &receive.sig.output else {
        return Error::new_spanned(&receive.sig, "receive must return Effect<Send>")
            .to_compile_error()
            .into();
    };
    let Type::Path(result_path) = result.as_ref() else {
        return Error::new_spanned(result, "receive must return Effect<Send>")
            .to_compile_error()
            .into();
    };
    let Some(effect) = result_path.path.segments.last() else {
        return Error::new_spanned(result, "receive must return Effect<Send>")
            .to_compile_error()
            .into();
    };
    if effect.ident != "Effect" {
        return Error::new_spanned(result, "receive must return Effect<Send>")
            .to_compile_error()
            .into();
    }
    let PathArguments::AngleBracketed(arguments) = &effect.arguments else {
        return Error::new_spanned(result, "Effect requires its send value type")
            .to_compile_error()
            .into();
    };
    let Some(GenericArgument::Type(send)) = arguments.args.first() else {
        return Error::new_spanned(result, "Effect requires its send value type")
            .to_compile_error()
            .into();
    };
    let behavior = match behavior_crate() {
        Ok(behavior) => behavior,
        Err(error) => return error.to_compile_error().into(),
    };
    let self_ty = &item.self_ty;
    let address = &from.ty;
    let message = &message.ty;
    let (impl_generics, _, where_clause) = item.generics.split_for_impl();

    quote! {
        #item

        impl #impl_generics #behavior::Protocol for #self_ty #where_clause {
            type Addr = #address;
            type Msg = #message;
        }

        impl #impl_generics #behavior::Behavior for #self_ty #where_clause {
            type Protocol = Self;
            type Event = #behavior::User<#address, #message>;
            type Sends = ::std::vec::Vec<#send>;
            type Ph = #behavior::Never;
            type Error = #behavior::Never;
            type Birth = #behavior::NoBirths;

            fn init(
                &mut self,
                _: #behavior::InitializationTurn,
            ) -> #behavior::BehaviorActed<Self> {
                ::core::result::Result::Ok(#behavior::Actions::cont())
            }

            fn transition(
                &mut self,
                _: #behavior::ActiveTurn,
                event: Self::Event,
            ) -> #behavior::BehaviorActed<Self> {
                ::core::result::Result::Ok(::core::convert::Into::into(
                    <#self_ty>::receive(self, event.from, event.message)
                ))
            }
        }

        impl #impl_generics #behavior::BehaviorBase for #self_ty #where_clause {
            type Base = Self;

            fn base(&self) -> &Self {
                self
            }
        }
    }
    .into()
}

/// Generate the mechanical `Behavior` implementation for a normal inherent
/// impl containing `receive(&mut self, from, message)` and, optionally,
/// `init(&mut self)`. Omitting `init` selects the behavior algebra's empty
/// initialization transition. The original impl and methods are preserved
/// unchanged.
#[proc_macro_attribute]
pub fn behavior(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as BehaviorArgs);
    let item = parse_macro_input!(item as ItemImpl);

    if item.trait_.is_some() {
        return Error::new_spanned(
            &item,
            "#[behavior] applies to an inherent impl, not a trait impl",
        )
        .to_compile_error()
        .into();
    }

    let init = item.items.iter().find_map(|item| match item {
        ImplItem::Fn(method) if method.sig.ident == "init" => Some(method),
        _ => None,
    });
    let receive = item.items.iter().find_map(|item| match item {
        ImplItem::Fn(method) if method.sig.ident == "receive" => Some(method),
        _ => None,
    });
    let Some(receive) = receive else {
        return Error::new_spanned(
            &item.self_ty,
            "#[behavior] requires a receive(&mut self, from, message) method",
        )
        .to_compile_error()
        .into();
    };
    if let Err(error) = init
        .map_or(Ok(()), validate_receiver)
        .and_then(|()| validate_receiver(receive))
    {
        return error.to_compile_error().into();
    }
    if let Some(init) = init
        && init.sig.inputs.len() != 1
    {
        return Error::new_spanned(&init.sig, "init must accept exactly &mut self")
            .to_compile_error()
            .into();
    }
    if receive.sig.inputs.len() != 3 {
        return Error::new_spanned(
            &receive.sig,
            "receive must accept exactly &mut self, from, and message",
        )
        .to_compile_error()
        .into();
    }

    let BehaviorArgs {
        addr,
        message,
        sends,
        births,
        error,
    } = args;
    let self_ty = &item.self_ty;
    let (impl_generics, _, where_clause) = item.generics.split_for_impl();
    let behavior = match behavior_crate() {
        Ok(behavior) => behavior,
        Err(error) => return error.to_compile_error().into(),
    };
    let initialize = init.map_or_else(
        || quote!(::core::result::Result::Ok(#behavior::Actions::cont())),
        |_| quote!(<#self_ty>::init(self)),
    );

    quote! {
        #item

        impl #impl_generics #behavior::Protocol for #self_ty #where_clause {
            type Addr = #addr;
            type Msg = #message;
        }

        impl #impl_generics #behavior::Behavior for #self_ty #where_clause {
            type Protocol = Self;
            type Event = #behavior::User<#addr, #message>;
            type Sends = #sends;
            type Ph = #behavior::Never;
            type Error = #error;
            type Birth = #births;

            fn init(
                &mut self,
                _: #behavior::InitializationTurn,
            ) -> #behavior::BehaviorActed<Self> {
                #initialize
            }

            fn transition(
                &mut self,
                _: #behavior::ActiveTurn,
                event: Self::Event,
            ) -> #behavior::BehaviorActed<Self> {
                <#self_ty>::receive(self, event.from, event.message)
            }
        }

        impl #impl_generics #behavior::BehaviorBase for #self_ty #where_clause {
            type Base = Self;

            fn base(&self) -> &Self {
                self
            }
        }
    }
    .into()
}
