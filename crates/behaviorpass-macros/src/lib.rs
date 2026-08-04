//! `behaviorpass-macros` — proc-macros for the behavior algebra.
//!
//! `workers!` compiles a mixed fleet declaration into the erasure-free sum
//! a `Supervising` fleet requires (design: actorpass docs, surface talk #2):
//! `(count, Type, build_fn)` per worker kind → a `Crew` enum with a
//! delegated `Behavior` impl, a per-variant range `crew_build`, and the
//! total count. `Crew` is a TYPE — every worker stays its own actor.
//!
//! v1 scope: every worker kind shares the SAME protocol (`Msg`, `Addr`,
//! `Error`, `Outbound`, `Offspring` — taken from the first kind). Mixed
//! protocols need the hand-written sum (the `CrewMsg` widening is a
//! deliberate, documented step — not this macro's job yet).

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Expr, LitInt, Result, Token, Type, parse_macro_input};

/// One `(count, Type, build_fn)` worker-kind spec.
struct Spec {
    count: LitInt,
    ty: Type,
    build: Expr,
}

impl Parse for Spec {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let count: Expr = content.parse()?;
        let Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(count), .. }) = count else {
            return Err(Error::new_spanned(count, "worker count must be a usize literal (ranges are computed at expansion)"));
        };
        content.parse::<Token![,]>()?;
        let ty: Type = content.parse()?;
        content.parse::<Token![,]>()?;
        let build: Expr = content.parse()?;
        Ok(Spec { count, ty, build })
    }
}

struct Specs(Punctuated<Spec, Token![,]>);

impl Parse for Specs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Specs(Punctuated::parse_terminated(input)?))
    }
}

/// `workers![(4, WorkerA, build_a), (2, WorkerB, build_b)]` → a block
/// declaring the `Crew` sum and yielding `(total, crew_build)` for
/// `Supervising`'s fleet. Slots are contiguous per variant (slot = nonce;
/// rest-for-one's birth order is the declaration order).
#[proc_macro]
pub fn workers(input: TokenStream) -> TokenStream {
    let Specs(specs) = parse_macro_input!(input as Specs);
    let specs: Vec<Spec> = specs.into_iter().collect();
    if specs.is_empty() {
        return Error::new(proc_macro2::Span::call_site(), "workers! needs at least one (count, Type, build_fn) spec")
            .to_compile_error()
            .into();
    }

    let first_ty = &specs[0].ty;
    let variants: Vec<_> = specs.iter().enumerate().map(|(i, _)| format_ident!("V{i}")).collect();
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
        build_arms.push(quote! { #start..#end => Crew::#v((#build)(i)) });
        start = end;
    }
    let total = start;

    let step_arms = variants.iter().map(|v| quote! { Crew::#v(b) => b.step(ev).await });
    let deadline_arms = variants.iter().map(|v| quote! { Crew::#v(b) => b.next_deadline() });
    let fleet_arms = variants.iter().map(|v| quote! { Crew::#v(b) => b.fleet() });

    let out = quote! {
        {
            /// The macro-generated mixed-fleet sum (see `workers!`).
            enum Crew {
                #(#variant_defs),*
            }

            impl ::behaviorpass::Behavior for Crew {
                type Addr = <#first_ty as ::behaviorpass::Behavior>::Addr;
                type Msg = <#first_ty as ::behaviorpass::Behavior>::Msg;
                type Ph = ::behaviorpass::Never;
                type Error = <#first_ty as ::behaviorpass::Behavior>::Error;
                type Outbound = <#first_ty as ::behaviorpass::Behavior>::Outbound;
                type Offspring = <#first_ty as ::behaviorpass::Behavior>::Offspring;

                async fn step(
                    &mut self,
                    ev: ::behaviorpass::Envelope<Self::Addr, Self::Msg>,
                ) -> ::behaviorpass::Acted<Self::Addr, Self::Ph, Self::Outbound, Self::Offspring, Self::Error> {
                    match self {
                        #(#step_arms),*
                    }
                }

                fn next_deadline(&self) -> ::core::option::Option<::tokio::time::Instant> {
                    match self {
                        #(#deadline_arms),*
                    }
                }

                fn fleet(&self) -> ::core::option::Option<::behaviorpass::Fleet<Self::Addr, Self::Offspring>> {
                    match self {
                        #(#fleet_arms),*
                    }
                }
            }

            fn crew_build(i: usize) -> Crew {
                match i {
                    #(#build_arms,)*
                    _ => unreachable!("workers!: fleet index out of range — driver/behavior desync"),
                }
            }

            (#total, crew_build as fn(usize) -> Crew)
        }
    };
    out.into()
}
