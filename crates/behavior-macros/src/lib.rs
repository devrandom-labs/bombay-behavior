//! `behavior-macros` — proc-macros for the behavior algebra.
//!
//! `#[behavior]` wires a fully declared inherent user-message fold and may
//! generate its nominal send and closed birth products.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::visit::Visit;
use syn::{
    Error, FnArg, GenericParam, Generics, Ident, ImplItem, ItemImpl, Result, ReturnType, Token,
    Type, braced, parse_macro_input, parse_quote,
};

fn crate_path(found: FoundCrate) -> TokenStream2 {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let name = syn::Ident::new(&name, Span::call_site());
            quote!(::#name)
        }
    }
}

fn facade_crate_path(found: FoundCrate) -> TokenStream2 {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            // Cargo names the package `bombay-rs`, while its public library
            // target is `bombay`. `proc_macro_crate` reports the normalized
            // package name for an unrenamed dependency, but generated Rust
            // must address the library target. An explicit dependency rename
            // remains the caller's actual extern-crate name.
            let name = if name == "bombay_rs" {
                "bombay".to_owned()
            } else {
                name
            };
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
    if std::env::var("CARGO_PKG_NAME").as_deref() == Ok("bombay-rs") {
        // `FoundCrate::Itself` identifies a package, but `crate` identifies
        // the target currently being compiled. The facade's library target is
        // `bombay`; sibling binaries and examples therefore reach its exports
        // through `::bombay`, not through their own `crate` root.
        return if std::env::var("CARGO_CRATE_NAME").as_deref() == Ok("bombay") {
            Ok(quote!(crate::behavior))
        } else {
            Ok(quote!(::bombay::behavior))
        };
    }
    if let Ok(found) = crate_name("bombay-behavior") {
        return Ok(crate_path(found));
    }
    if let Ok(found) = crate_name("bombay-rs") {
        let bombay = facade_crate_path(found);
        return Ok(quote!(#bombay::behavior));
    }
    Err(Error::new(
        Span::call_site(),
        "could not resolve `bombay-behavior` directly or through `bombay-rs`",
    ))
}

#[cfg(test)]
mod crate_resolution_tests {
    use super::*;

    #[test]
    fn facade_default_library_name_and_explicit_rename_resolve_distinctly() {
        assert_eq!(
            facade_crate_path(FoundCrate::Name("bombay_rs".to_owned())).to_string(),
            ":: bombay"
        );
        assert_eq!(
            facade_crate_path(FoundCrate::Name("runtime".to_owned())).to_string(),
            ":: runtime"
        );
    }
}

struct NamedField {
    name: Ident,
    ty: Type,
}

struct NamedProduct {
    fields: Vec<NamedField>,
}

enum SendsSpec {
    Existing(Type),
    Generated(NamedProduct),
}

enum BirthsSpec {
    Existing(Type),
    Generated(NamedProduct),
}

fn parse_product(input: ParseStream) -> Result<NamedProduct> {
    let content;
    braced!(content in input);
    let mut fields = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    while !content.is_empty() {
        let name: Ident = content.parse()?;
        if !names.insert(name.to_string()) {
            return Err(Error::new_spanned(name, "duplicate product lane"));
        }
        content.parse::<Token![:]>()?;
        fields.push(NamedField {
            name,
            ty: content.parse()?,
        });
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else if !content.is_empty() {
            return Err(content.error("expected `,` between product lanes"));
        }
    }
    if fields.is_empty() {
        return Err(input.error("empty products must be omitted"));
    }
    Ok(NamedProduct { fields })
}

struct BehaviorArgs {
    addr: Type,
    message: Type,
    sends: Option<SendsSpec>,
    births: Option<BirthsSpec>,
    error: Option<Type>,
}

impl Parse for BehaviorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut addr = None;
        let mut message = None;
        let mut sends = None;
        let mut births = None;
        let mut error = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "addr" if addr.is_none() => addr = Some(input.parse()?),
                "message" if message.is_none() => message = Some(input.parse()?),
                "sends" if sends.is_none() => {
                    sends = Some(if input.peek(syn::token::Brace) {
                        SendsSpec::Generated(parse_product(input)?)
                    } else {
                        SendsSpec::Existing(input.parse()?)
                    });
                }
                "births" if births.is_none() => {
                    births = Some(if input.peek(syn::token::Brace) {
                        BirthsSpec::Generated(parse_product(input)?)
                    } else {
                        BirthsSpec::Existing(input.parse()?)
                    });
                }
                "error" if error.is_none() => error = Some(input.parse()?),
                "addr" | "message" | "sends" | "births" | "error" => {
                    return Err(Error::new_spanned(key, "duplicate behavior argument"));
                }
                _ => return Err(Error::new_spanned(key, "unknown behavior argument")),
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            } else if !input.is_empty() {
                return Err(input.error("expected `,` between behavior arguments"));
            }
        }
        Ok(Self {
            addr: addr.ok_or_else(|| input.error("missing `addr`"))?,
            message: message.ok_or_else(|| input.error("missing `message`"))?,
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

fn pascal_name(field: &Ident) -> String {
    field
        .to_string()
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<String>()
}

fn lane_name(product: &Ident, field: &Ident) -> Ident {
    let lane = pascal_name(field);
    format_ident!("{}{}", product, lane)
}

#[derive(Default)]
struct GenericUses {
    identifiers: std::collections::BTreeSet<String>,
    lifetimes: std::collections::BTreeSet<String>,
}

impl<'ast> Visit<'ast> for GenericUses {
    fn visit_ident(&mut self, ident: &'ast Ident) {
        self.identifiers.insert(ident.to_string());
    }

    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        self.lifetimes.insert(lifetime.ident.to_string());
    }
}

fn product_generics(item: &ItemImpl, fields: &[NamedField]) -> Generics {
    let mut uses = GenericUses::default();
    for field in fields {
        uses.visit_type(&field.ty);
    }
    let mut generics = item.generics.clone();
    generics.params = generics
        .params
        .into_iter()
        .filter(|parameter| match parameter {
            GenericParam::Lifetime(parameter) => uses
                .lifetimes
                .contains(&parameter.lifetime.ident.to_string()),
            GenericParam::Type(parameter) => {
                uses.identifiers.contains(&parameter.ident.to_string())
            }
            GenericParam::Const(parameter) => {
                uses.identifiers.contains(&parameter.ident.to_string())
            }
        })
        .collect();
    generics.where_clause = None;
    generics
}

#[allow(
    clippy::too_many_lines,
    reason = "the generated nominal product keeps every structural trait implementation adjacent"
)]
fn generate_sends(
    product: Option<SendsSpec>,
    actor: &Ident,
    item: &ItemImpl,
    behavior: &TokenStream2,
) -> (TokenStream2, TokenStream2) {
    let product = match product {
        None => return (quote!(#behavior::NoSends), quote!()),
        Some(SendsSpec::Existing(ty)) => return (quote!(#ty), quote!()),
        Some(SendsSpec::Generated(product)) => product,
    };
    let name = format_ident!("{}Sends", actor);
    let field_names: Vec<_> = product.fields.iter().map(|field| &field.name).collect();
    let field_types: Vec<_> = product.fields.iter().map(|field| &field.ty).collect();
    let lane_names: Vec<_> = field_names
        .iter()
        .map(|field| lane_name(&name, field))
        .collect();
    let actions_name = format_ident!("{}Actions", actor);
    let action_methods: Vec<_> = field_names
        .iter()
        .map(|field| {
            let field = field.to_string();
            format_ident!("send_{}", field.trim_start_matches("r#"))
        })
        .collect();
    let product_generics = product_generics(item, &product.fields);
    let (_, type_generics, _) = product_generics.split_for_impl();
    let generics = &product_generics;
    let actor_type = &item.self_ty;
    let mut action_trait_generics = product_generics.clone();
    action_trait_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#actor_type: ::core::marker::Sized));

    let action_trait_methods =
        action_methods
            .iter()
            .zip(lane_names.iter())
            .map(|(method, lane)| {
                quote! {
                    /// Append one value to this generated semantic send lane.
                    #[must_use]
                    fn #method<__BombayInput>(self, input: __BombayInput) -> Self
                    where
                        Self: #behavior::AppendSend<__BombayInput, #lane>;
                }
            });

    let action_impl_methods = action_methods
        .iter()
        .zip(lane_names.iter())
        .map(|(method, lane)| {
            quote! {
                fn #method<__BombayInput>(self, input: __BombayInput) -> Self
                where
                    Self: #behavior::AppendSend<__BombayInput, #lane>,
                {
                    <Self as #behavior::AppendSend<__BombayInput, #lane>>::append_send(
                        self,
                        input,
                    )
                }
            }
        });

    let mut actions_impl_generics = product_generics.clone();
    actions_impl_generics
        .params
        .push(parse_quote!(__BombayAddress));
    actions_impl_generics.params.push(parse_quote!(__BombayPh));
    actions_impl_generics
        .params
        .push(parse_quote!(__BombayBirth));
    actions_impl_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(__BombayAddress: #behavior::Address));
    actions_impl_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(__BombayBirth: #behavior::BirthMode));
    actions_impl_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#actor_type: ::core::marker::Sized));
    let (actions_impl_generics, _, actions_where_clause) = actions_impl_generics.split_for_impl();

    let send_impls = lane_names
        .iter()
        .zip(field_names.iter())
        .zip(field_types.iter())
        .map(|((lane, field), field_ty)| {
            let mut generics = product_generics.clone();
            generics.params.push(parse_quote!(__BombayInput));
            generics.make_where_clause().predicates.push(parse_quote!(
                #field_ty: #behavior::SendInput<__BombayInput, #behavior::Own>
            ));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            quote! {
                impl #impl_generics #behavior::SendInput<__BombayInput, #lane>
                    for #name #type_generics #where_clause
                {
                    fn emit(&mut self, input: __BombayInput) {
                        <#field_ty as #behavior::SendInput<
                            __BombayInput,
                            #behavior::Own,
                        >>::emit(&mut self.#field, input);
                    }
                }
            }
        });

    let mut effects_generics = product_generics.clone();
    for field_ty in &field_types {
        effects_generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#field_ty: #behavior::SendEffects));
    }
    let (effects_impl_generics, _, effects_where_clause) = effects_generics.split_for_impl();

    let mut lawful_generics = product_generics.clone();
    lawful_generics.params.push(parse_quote!(__BombayEvent));
    for field_ty in &field_types {
        lawful_generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#field_ty: #behavior::SendsFor<__BombayEvent>));
    }
    let (lawful_impl_generics, _, lawful_where_clause) = lawful_generics.split_for_impl();

    let mut interpret_generics = product_generics.clone();
    interpret_generics
        .params
        .push(parse_quote!(__BombayInterpreter));
    interpret_generics
        .params
        .push(parse_quote!(__BombayRootEvent));
    interpret_generics.params.push(parse_quote!(__BombayPath));
    interpret_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(__BombayInterpreter: #behavior::SendInterpreter));
    for field_ty in &field_types {
        interpret_generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(
                #field_ty: #behavior::InterpretSends<
                    __BombayInterpreter,
                    __BombayRootEvent,
                    __BombayPath,
                >
            ));
    }
    interpret_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#name #type_generics: ::core::marker::Send));
    let (interpret_impl_generics, _, interpret_where_clause) = interpret_generics.split_for_impl();

    let items = quote! {
        #(pub enum #lane_names {})*

        pub struct #name #generics {
            #(pub #field_names: #field_types,)*
        }

        /// Fluent, statically routed send-lane methods for this behavior's
        /// generated action product.
        pub trait #actions_name #action_trait_generics: ::core::marker::Sized {
            #(#action_trait_methods)*
        }

        impl #actions_impl_generics #actions_name #type_generics
            for #behavior::Actions<
                __BombayAddress,
                __BombayPh,
                #name #type_generics,
                __BombayBirth,
            >
            #actions_where_clause
        {
            #(#action_impl_methods)*
        }

        impl #effects_impl_generics #behavior::SendEffects for #name #type_generics
            #effects_where_clause
        {
            fn empty() -> Self {
                Self {
                    #(#field_names: <#field_types as #behavior::SendEffects>::empty(),)*
                }
            }

            fn append(&mut self, other: Self) {
                #(
                    <#field_types as #behavior::SendEffects>::append(
                        &mut self.#field_names,
                        other.#field_names,
                    );
                )*
            }
        }

        impl #lawful_impl_generics #behavior::SendsFor<__BombayEvent>
            for #name #type_generics #lawful_where_clause
        {}

        #(#send_impls)*

        impl #interpret_impl_generics
            #behavior::InterpretSends<__BombayInterpreter, __BombayRootEvent, __BombayPath>
            for #name #type_generics #interpret_where_clause
        {
            fn interpret(
                self,
                interpreter: &mut __BombayInterpreter,
            ) -> impl ::core::future::Future<
                Output = ::core::result::Result<(), __BombayInterpreter::Error>,
            > + ::core::marker::Send {
                async move {
                    #(
                        <#field_types as #behavior::InterpretSends<
                            __BombayInterpreter,
                            __BombayRootEvent,
                            __BombayPath,
                        >>::interpret(self.#field_names, interpreter).await?;
                    )*
                    ::core::result::Result::Ok(())
                }
            }
        }
    };
    (quote!(#name #type_generics), items)
}

fn generate_births(
    product: Option<BirthsSpec>,
    actor: &Ident,
    addr: &Type,
    item: &ItemImpl,
    behavior: &TokenStream2,
) -> (TokenStream2, TokenStream2) {
    let product = match product {
        None => return (quote!(#behavior::NoBirths), quote!()),
        Some(BirthsSpec::Existing(ty)) => return (quote!(#ty), quote!()),
        Some(BirthsSpec::Generated(product)) => product,
    };
    let name = format_ident!("{}Children", actor);
    let roles_name = format_ident!("{}Child", actor);
    let routes_name = format_ident!("{}ChildrenRoutes", actor);
    let field_names = product
        .fields
        .iter()
        .map(|field| &field.name)
        .collect::<Vec<_>>();
    let role_names = product
        .fields
        .iter()
        .map(|field| lane_name(&name, &field.name))
        .collect::<Vec<_>>();
    let role_values = product
        .fields
        .iter()
        .map(|field| format_ident!("{}", pascal_name(&field.name)))
        .collect::<Vec<_>>();
    let declared_child_types = product
        .fields
        .iter()
        .map(|field| &field.ty)
        .collect::<Vec<_>>();
    let role_positions = (0..product.fields.len())
        .map(|index| {
            (0..(product.fields.len() - index - 1)).fold(
                quote!(#behavior::ChildHead),
                |position, _| quote!(#behavior::ChildTail<#position>),
            )
        })
        .collect::<Vec<_>>();
    let child_types = product.fields.iter().map(|field| &field.ty);
    let product_generics = product_generics(item, &product.fields);
    let (impl_generics, type_generics, where_clause) = product_generics.split_for_impl();
    let generics = &product_generics;
    let parent = &item.self_ty;
    let (parent_impl_generics, _, parent_where_clause) = item.generics.split_for_impl();
    let choice = child_types.fold(
        quote!(#behavior::Never),
        |tail, child| quote!(#behavior::ChildChoice<#child, #tail>),
    );
    (
        quote!(#behavior::Births<#name #type_generics>),
        quote! {
            pub type #name #generics = #choice;

            #(
                #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
                pub struct #role_names;

                impl #parent_impl_generics #behavior::ChildRole<#parent> for #role_names
                    #parent_where_clause
                {
                    type Child = #declared_child_types;
                    type Position = #role_positions;
                }

                impl #parent_impl_generics #behavior::ChildOccurrence<#parent> for #role_names
                    #parent_where_clause
                {
                    type Resolution = #behavior::DeclaredChildOccurrence;
                }
            )*

            pub struct #roles_name;

            #[allow(non_upper_case_globals)]
            impl #roles_name {
                #(pub const #role_values: #role_names = #role_names;)*
            }

            pub struct #routes_name #generics {
                #(
                    pub #field_names: #behavior::ChildRoute<#declared_child_types, #role_names>,
                )*
            }

            impl #impl_generics #routes_name #type_generics #where_clause {
                #[must_use]
                pub fn new(
                    #(#field_names: <#addr as #behavior::Address>::Nonce,)*
                ) -> Self {
                    Self {
                        #(
                            #field_names: #behavior::ChildRoute::new(#field_names),
                        )*
                    }
                }
            }
        },
    )
}

/// Generate the mechanical `Behavior` implementation for a normal inherent
/// impl containing `receive(&mut self, from, message)` and, optionally,
/// `init(&mut self)`. Omitting `init` selects the behavior algebra's empty
/// initialization transition. The original impl and methods are preserved
/// unchanged.
#[proc_macro_attribute]
#[allow(
    clippy::too_many_lines,
    reason = "validation and the one coherent Behavior expansion remain in one entry point"
)]
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
    let Type::Path(self_path) = self_ty.as_ref() else {
        return Error::new_spanned(self_ty, "#[behavior] requires a nominal actor type")
            .to_compile_error()
            .into();
    };
    let Some(self_name) = self_path.path.segments.last().map(|segment| &segment.ident) else {
        return Error::new_spanned(self_ty, "#[behavior] requires a nominal actor type")
            .to_compile_error()
            .into();
    };
    let (impl_generics, _, where_clause) = item.generics.split_for_impl();
    let behavior = match behavior_crate() {
        Ok(behavior) => behavior,
        Err(error) => return error.to_compile_error().into(),
    };
    let error = error.map_or_else(|| quote!(#behavior::Never), |error| quote!(#error));
    let initialize = init.map_or_else(
        || quote!(::core::result::Result::Ok(#behavior::Actions::cont())),
        |_| quote!(<#self_ty>::init(self)),
    );

    let (sends_ty, sends_items) = generate_sends(sends, self_name, &item, &behavior);
    let (births_ty, births_items) = generate_births(births, self_name, &addr, &item, &behavior);

    quote! {
        #sends_items
        #births_items
        #item

        impl #impl_generics #behavior::Protocol for #self_ty #where_clause {
            type Addr = #addr;
            type Msg = #message;
        }

        impl #impl_generics #behavior::Behavior for #self_ty #where_clause {
            type Protocol = Self;
            type Event = #behavior::User<#addr, #message>;
            type Sends = #sends_ty;
            type Ph = #behavior::Never;
            type Error = #error;
            type Birth = #births_ty;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn permutations(values: &mut [&str], at: usize, output: &mut Vec<String>) {
        if at == values.len() {
            output.push(values.join(", "));
            return;
        }
        for index in at..values.len() {
            values.swap(at, index);
            permutations(values, at + 1, output);
            values.swap(at, index);
        }
    }

    #[test]
    fn every_argument_order_parses() {
        let mut declarations = [
            "addr = u64",
            "message = String",
            "sends = { first: Vec<u8>, second: Vec<u16> }",
            "births = { first: u8, second: u16 }",
            "error = String",
        ];
        let mut cases = Vec::new();
        permutations(&mut declarations, 0, &mut cases);
        assert_eq!(cases.len(), 120);
        for case in cases {
            syn::parse_str::<BehaviorArgs>(&case)
                .unwrap_or_else(|error| panic!("failed to parse `{case}`: {error}"));
        }
    }

    #[test]
    fn capability_omission_and_trailing_comma_parse() {
        syn::parse_str::<BehaviorArgs>("message = (), addr = u8,")
            .expect("only required protocol declarations are sufficient");
    }

    #[test]
    fn invalid_declaration_states_are_rejected() {
        for case in [
            "message = ()",
            "addr = u8",
            "addr = u8, message = (), addr = u16",
            "addr = u8, message = (), unknown = u8",
            "addr = u8, message = (), sends = {}",
            "addr = u8, message = (), births = {}",
            "addr = u8, message = (), sends = { same: Vec<u8>, same: Vec<u8> }",
            "addr = u8, message = (), births = { same: u8, same: u16 }",
        ] {
            assert!(
                syn::parse_str::<BehaviorArgs>(case).is_err(),
                "invalid declaration parsed: `{case}`"
            );
        }
    }
}
