//! `behavior-macros` — proc-macros for the behavior algebra.
//!
//! `#[behavior]` wires an ordinary inherent user-message fold to the concrete
//! `Behavior` trait. `#[births]` wires a closed creation-only enum to exhaustive
//! static installation; it never generates a behavior or forwarding protocol.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    Error, Fields, FnArg, ImplItem, ItemEnum, ItemImpl, Result, ReturnType, Token, Type,
    parse_macro_input,
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

        impl #impl_generics #behavior::Behavior for #self_ty #where_clause {
            type Addr = #addr;
            type Msg = #message;
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

/// Generate exhaustive static installation dispatch for a closed,
/// creation-only heterogeneous child sum.
#[proc_macro_attribute]
pub fn births(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return Error::new(Span::call_site(), "#[births] accepts no arguments")
            .to_compile_error()
            .into();
    }
    let item = parse_macro_input!(item as ItemEnum);
    if item.variants.is_empty() {
        return Error::new_spanned(&item, "a birth sum must contain at least one variant")
            .to_compile_error()
            .into();
    }
    let mut variants = Vec::new();
    for variant in &item.variants {
        if variant.discriminant.is_some() {
            return Error::new_spanned(variant, "birth variants cannot have discriminants")
                .to_compile_error()
                .into();
        }
        let Fields::Unnamed(fields) = &variant.fields else {
            return Error::new_spanned(
                variant,
                "each birth variant must contain exactly one unnamed child type",
            )
            .to_compile_error()
            .into();
        };
        if fields.unnamed.len() != 1 {
            return Error::new_spanned(
                variant,
                "each birth variant must contain exactly one unnamed child type",
            )
            .to_compile_error()
            .into();
        }
        variants.push((variant.ident.clone(), fields.unnamed[0].ty.clone()));
    }

    let behavior = match behavior_crate() {
        Ok(behavior) => behavior,
        Err(error) => return error.to_compile_error().into(),
    };
    let name = &item.ident;
    let (_, type_generics, _) = item.generics.split_for_impl();
    let variant_names: Vec<_> = variants.iter().map(|(name, _)| name).collect();
    let child_types: Vec<_> = variants.iter().map(|(_, child)| child).collect();
    let mut dispatch_generics = item.generics.clone();
    dispatch_generics.params.push(syn::parse_quote!(__A));
    dispatch_generics
        .params
        .push(syn::parse_quote!(__Installer));
    dispatch_generics.params.push(syn::parse_quote!(__Output));
    dispatch_generics.params.push(syn::parse_quote!(__Error));
    let dispatch_where = dispatch_generics.make_where_clause();
    dispatch_where
        .predicates
        .push(syn::parse_quote!(__A: #behavior::Address));
    dispatch_where.predicates.push(syn::parse_quote!(
        <__A as #behavior::Address>::Nonce: ::core::marker::Send
    ));
    dispatch_where
        .predicates
        .push(syn::parse_quote!(__Installer: ::core::marker::Send));
    for child in &child_types {
        dispatch_where.predicates.push(
            syn::parse_quote!(#child: #behavior::Behavior<Addr = __A> + ::core::marker::Send),
        );
        dispatch_where.predicates.push(syn::parse_quote!(
            __Installer: #behavior::InstallBirth<__A, #child, __Output, __Error>
        ));
    }
    let (dispatch_impl_generics, _, dispatch_where_clause) = dispatch_generics.split_for_impl();

    quote! {
        #item

        impl #dispatch_impl_generics
            #behavior::DispatchBirth<__A, __Installer, __Output, __Error>
            for #name #type_generics
            #dispatch_where_clause
        {
            async fn dispatch_birth(
                self,
                nonce: <__A as #behavior::Address>::Nonce,
                kind: #behavior::CreationKind<<__A as #behavior::Address>::Nonce>,
                installer: &mut __Installer,
            ) -> ::core::result::Result<__Output, __Error> {
                match self {
                    #(
                        Self::#variant_names(child) => {
                            #behavior::InstallBirth::install_birth(
                                installer,
                                #behavior::Create::new(nonce, child, kind),
                            ).await
                        }
                    )*
                }
            }
        }
    }
    .into()
}
