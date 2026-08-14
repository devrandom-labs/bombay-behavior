//! `behavior-macros` — proc-macros for the behavior algebra.
//!
//! `workers!` compiles a mixed fleet declaration into the erasure-free sum
//! a `Supervisor` fleet requires (design: actorpass docs, surface talk #2):
//! `(count, Type, build_fn)` per worker kind → a `Worker` enum with a
//! delegated `Behavior` impl, a per-variant range `build_worker`, and the
//! total count. `Worker` is a type—every worker stays its own actor.
//!
//! v1 scope: every worker kind shares the SAME protocol (`Event`, `Sends`,
//! `Error`, and `Birth` — taken from the first kind). Mixed
//! protocols need the hand-written sum (the `WorkerMsg` widening is a
//! deliberate, documented step — not this macro's job yet).

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Error, Expr, FnArg, ImplItem, ItemImpl, LitInt, Result, ReturnType, Token, Type,
    parse_macro_input,
};

mod behavior_kw {
    syn::custom_keyword!(addr);
    syn::custom_keyword!(message);
    syn::custom_keyword!(sends);
    syn::custom_keyword!(births);
    syn::custom_keyword!(error);
}

fn behavior_crate() -> Result<proc_macro2::TokenStream> {
    if std::env::var("CARGO_PKG_NAME").as_deref() == Ok("bombay-behavior") {
        // This package deliberately exposes the library target as `behavior`,
        // not Cargo's normalized package name `bombay_behavior`. The same path
        // works in its unit, integration, and rustdoc crates.
        return Ok(quote!(::behavior));
    }
    match crate_name("bombay-behavior") {
        Ok(FoundCrate::Itself) => Ok(quote!(::behavior)),
        Ok(FoundCrate::Name(name)) => {
            let name = syn::Ident::new(&name, Span::call_site());
            Ok(quote!(::#name))
        }
        Err(error) => Err(Error::new(
            Span::call_site(),
            format!("could not resolve the bombay-behavior crate: {error}"),
        )),
    }
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

/// One `(count, Type, build_fn)` worker-kind spec.
struct Compose {
    count: LitInt,
    ty: Type,
    build: Expr,
}

impl Parse for Compose {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let count: Expr = content.parse()?;
        let Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(count),
            ..
        }) = count
        else {
            return Err(Error::new_spanned(
                count,
                "worker count must be a usize literal (ranges are computed at expansion)",
            ));
        };
        content.parse::<Token![,]>()?;
        let ty: Type = content.parse()?;
        content.parse::<Token![,]>()?;
        let build: Expr = content.parse()?;
        Ok(Compose { count, ty, build })
    }
}

struct Specs(Punctuated<Compose, Token![,]>);

impl Parse for Specs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Specs(Punctuated::parse_terminated(input)?))
    }
}

/// `workers![(4, WorkerA, build_a), (2, WorkerB, build_b)]` → a block
/// declaring the `Worker` sum and yielding `(total, build_worker)` for
/// `Supervisor`'s fleet. Slots are contiguous per variant (slot = nonce;
/// rest-for-one's birth order is the declaration order).
#[proc_macro]
pub fn workers(input: TokenStream) -> TokenStream {
    let Specs(specs) = parse_macro_input!(input as Specs);
    let specs: Vec<Compose> = specs.into_iter().collect();
    if specs.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "workers! needs at least one (count, Type, build_fn) spec",
        )
        .to_compile_error()
        .into();
    }

    let first_ty = &specs[0].ty;
    let behavior = match behavior_crate() {
        Ok(behavior) => behavior,
        Err(error) => return error.to_compile_error().into(),
    };
    let variants: Vec<_> = specs
        .iter()
        .enumerate()
        .map(|(i, _)| format_ident!("V{i}"))
        .collect();
    let variant_defs = specs.iter().zip(&variants).map(|(s, v)| {
        let ty = &s.ty;
        quote! { #v(#ty) }
    });

    let mut start = 0_usize;
    let mut build_arms = Vec::new();
    for (s, v) in specs.iter().zip(&variants) {
        let n: usize = match s.count.base10_parse() {
            Ok(n) => n,
            Err(e) => return e.to_compile_error().into(),
        };
        let end = start + n;
        let build = &s.build;
        build_arms
            .push(quote! { #start..#end => ::core::option::Option::Some(Worker::#v((#build)(i))) });
        start = end;
    }
    let total = start;

    let step_arms = variants
        .iter()
        .map(|v| quote! { Worker::#v(b) => b.transition(turn, ev) });
    let init_arms = variants
        .iter()
        .map(|v| quote! { Worker::#v(b) => b.init(turn) });

    let out = quote! {
        {
            /// The macro-generated mixed-fleet sum (see `workers!`).
            enum Worker {
                #(#variant_defs),*
            }

            impl #behavior::Behavior for Worker {
                type Addr = <#first_ty as #behavior::Behavior>::Addr;
                type Msg = <#first_ty as #behavior::Behavior>::Msg;
                type Event = <#first_ty as #behavior::Behavior>::Event;
                type Sends = <#first_ty as #behavior::Behavior>::Sends;
                type Ph = #behavior::Never;
                type Error = <#first_ty as #behavior::Behavior>::Error;
                type Birth = <#first_ty as #behavior::Behavior>::Birth;

                fn init(&mut self, turn: #behavior::InitializationTurn) -> ::core::result::Result<
                    #behavior::Actions<Self::Addr, Self::Ph, Self::Sends, Self::Birth>,
                    Self::Error,
                > {
                    match self {
                        #(#init_arms),*
                    }
                }

                fn transition(
                    &mut self,
                    turn: #behavior::ActiveTurn,
                    ev: Self::Event,
                ) -> ::core::result::Result<
                    #behavior::Actions<Self::Addr, Self::Ph, Self::Sends, Self::Birth>,
                    Self::Error,
                > {
                    match self {
                        #(#step_arms),*
                    }
                }

            }

            fn build_worker(i: usize) -> ::core::option::Option<Worker> {
                match i {
                    #(#build_arms,)*
                    _ => ::core::option::Option::None,
                }
            }

            (#total, build_worker as fn(usize) -> ::core::option::Option<Worker>)
        }
    };
    out.into()
}
