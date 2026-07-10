//! The [`tron_sol!`] procedural macro — a TRON-aware superset of alloy's `sol!`.
//!
//! `tron_sol!` forwards its entire input to `alloy_sol_types::sol!` to generate
//! the Solidity type layer (`…Call` structs, events, errors, custom types,
//! free-standing `struct`/`enum`/`type` definitions, …) and *additionally*
//! generates a provider-bound `Instance` for every `contract`/`interface`
//! carrying `#[sol(rpc)]`, wired to `tronz`'s `TronProvider`.
//!
//! It accepts everything `sol!` does for the inline-Solidity form, including:
//!
//! - **multiple items in one invocation** (several contracts, or contracts mixed with bare
//!   `struct`/`enum`/`error`/`event`/`type` definitions);
//! - **attribute passthrough**: any attribute other than the TRON-specific ones (`#[sol(rpc)]`,
//!   `#[sol(bytecode = …)]`, `#[sol(deployed_bytecode = …)]`, `#[tron_sol(…)]`) is forwarded
//!   verbatim to `sol!` — so `#[derive(…)]`, `#[sol(all_derives)]`, `#[sol(extra_derives(…))]`, doc
//!   comments, etc. all work on the generated type layer.
//!
//! TRON-specific attributes:
//!
//! - `#[sol(rpc)]` — also generate a `TronProvider`-bound `Instance`.
//! - `#[sol(bytecode = "0x…")]` — embed creation bytecode and generate `deploy_builder` / `deploy`
//!   helpers (requires `#[sol(rpc)]`).
//! - `#[sol(deployed_bytecode = "0x…")]` — embed the runtime bytecode as a `DEPLOYED_BYTECODE`
//!   constant.
//! - `#[tron_sol(tronz_crate = <path>)]` — override the runtime crate path.
//!
//! ```ignore
//! // Type layer only (same as sol!) — bare types and multiple items are fine.
//! tron_sol! {
//!     struct Foo { uint256 x; }
//!     enum Bar { A, B }
//! }
//!
//! // Type layer + TRON RPC bindings, with derive passthrough.
//! tron_sol! {
//!     #[derive(Debug)]
//!     #[sol(rpc)]
//!     interface IERC20 {
//!         function balanceOf(address owner) external view returns (uint256);
//!         function transfer(address to, uint256 amount) external returns (bool);
//!     }
//! }
//!
//! let contract = IERC20::new(usdt_addr, provider);
//! let balance = contract.balanceOf(owner).call().await?;
//! ```

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    iter::once,
};

use proc_macro::TokenStream;
use proc_macro2::{
    Delimiter, Group, Ident as Ident2, Punct, Spacing, Span, TokenStream as TokenStream2, TokenTree,
};
use quote::{format_ident, quote};
use syn::{
    Attribute, Ident, LitStr, Result,
    parse::{Parse, ParseStream},
    parse_macro_input,
};
use syn_solidity::{
    FunctionKind, Item as SolItem, ItemContract, ItemEvent, ItemFunction, Spanned, Type as SolType,
};

/// A TRON-aware superset of alloy's `sol!`. See the [crate-level docs](crate).
#[proc_macro]
pub fn tron_sol(input: TokenStream) -> TokenStream {
    let original = TokenStream2::from(input.clone());
    let parsed = parse_macro_input!(input as TronSol);
    match parsed.expand(original) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

struct TronSol {
    items: Vec<SolItem>,
    krate: TokenStream2,
}

impl Parse for TronSol {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse::<SolItem>()?);
        }

        // `#[tron_sol(tronz_crate = <path>)]` may appear on any item; the last
        // one wins. Defaults to the umbrella crate's `contract` module.
        let mut krate: TokenStream2 = quote!(::tronz::contract);
        for item in &items {
            let Some(attrs) = item.attrs() else { continue };
            for attr in attrs {
                if attr.path().is_ident("tron_sol") {
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("tronz_crate") {
                            let path: syn::Path = meta.value()?.parse()?;
                            krate = quote!(#path);
                            Ok(())
                        } else {
                            Err(meta.error("unknown `tron_sol` option; expected `tronz_crate`"))
                        }
                    })?;
                }
            }
        }

        Ok(Self { items, krate })
    }
}

impl TronSol {
    fn expand(&self, original: TokenStream2) -> Result<TokenStream2> {
        // All runtime paths go through `__private` to avoid a direct dependency
        // on tronz-contract from this proc-macro crate.
        let kpriv = {
            let k = &self.krate;
            quote!(#k::__private)
        };
        let alloy = quote!(#kpriv::alloy_sol_types);

        // Compute a hash of the raw input before it is consumed, so the hidden
        // types module gets a unique name regardless of contract names used.
        let input_hash = {
            let mut hasher = DefaultHasher::new();
            original.to_string().hash(&mut hasher);
            hasher.finish()
        };

        // The full type layer, with TRON-specific attributes stripped so alloy's
        // `sol!` sees only what it understands.
        let forwarded = strip_tron_attrs(original);

        let mut rpc_contracts: Vec<&ItemContract> = Vec::new();
        for item in &self.items {
            if let SolItem::Contract(c) = item {
                if contract_opts(&c.attrs)?.rpc {
                    rpc_contracts.push(c);
                }
            }
        }

        // No RPC layer requested — emit the type layer directly (closest to a
        // plain `sol!`), supporting multiple items and bare type definitions.
        if rpc_contracts.is_empty() {
            return Ok(quote! {
                #alloy::sol! {
                    #![sol(alloy_sol_types = #alloy)]
                    #forwarded
                }
            });
        }

        // RPC layer requested. Put the whole type layer in a hidden module and
        // re-export it; then add one `Instance` module per RPC contract. The
        // explicit `pub mod <Name>` shadows the glob-imported contract module of
        // the same name (an explicit item always shadows a glob import).
        //
        // The hidden module name includes a hash of the entire input so that
        // multiple `tron_sol!` invocations in the same scope never collide, even
        // when they happen to share the same first contract name.
        let types_mod = format_ident!("__tron_sol_types_{:x}", input_hash);

        let mut instances = Vec::new();
        for c in &rpc_contracts {
            instances.push(self.expand_contract(c, &kpriv, &types_mod)?);
        }

        Ok(quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types, non_snake_case, missing_docs, clippy::all)]
            mod #types_mod {
                #alloy::sol! {
                    #![sol(alloy_sol_types = #alloy)]
                    #forwarded
                }
            }

            #[allow(unused_imports)]
            pub use #types_mod::*;

            #(#instances)*
        })
    }

    /// Generate the provider-bound `Instance` module for one `#[sol(rpc)]`
    /// contract.
    fn expand_contract(
        &self,
        c: &ItemContract,
        kpriv: &TokenStream2,
        types_mod: &Ident,
    ) -> Result<TokenStream2> {
        let name = c.name.0.clone();
        let opts = contract_opts(&c.attrs)?;

        if opts.rename {
            return Err(syn::Error::new(
                c.name.span(),
                "`#[sol(rename/rename_all)]` is not supported together with `#[sol(rpc)]`: \
                 renaming the generated `…Call` types would desync the instance methods",
            ));
        }

        let alloy = quote!(#kpriv::alloy_sol_types);
        let aprim = quote!(#kpriv::alloy_primitives);
        let taddr = quote!(#kpriv::tronz_primitives::Address);
        let provider_tr = quote!(#kpriv::tronz_provider::TronProvider);
        let cinst = quote!(#kpriv::ContractInstance);
        let tcb = quote!(#kpriv::TronCallBuilder);
        let tef = quote!(#kpriv::TronEventFilter);
        let deploy_builder_ty = quote!(#kpriv::DeployBuilder);
        let result_ty = quote!(#kpriv::Result);

        // Split the contract body into callable functions, constructor, and events.
        // Public state variables are converted to getter functions via
        // `ItemFunction::from_variable_definition`, matching alloy's behaviour.
        let mut functions: Vec<ItemFunction> = Vec::new();
        let mut constructor: Option<ItemFunction> = None;
        let mut events: Vec<ItemEvent> = Vec::new();
        for item in &c.body {
            match item {
                SolItem::Function(f) => {
                    if matches!(f.kind, FunctionKind::Function(_)) && f.name.is_some() {
                        functions.push(f.clone());
                    } else if matches!(f.kind, FunctionKind::Constructor(_)) {
                        constructor = Some(f.clone());
                    }
                }
                SolItem::Variable(v) => {
                    // Public state variables expose a getter — convert to a synthetic
                    // function and treat it exactly like an explicit `function` item.
                    if v.attributes.visibility().is_some_and(|vis| vis.is_public()) {
                        functions.push(ItemFunction::from_variable_definition(v.clone()));
                    }
                }
                SolItem::Event(e) => {
                    events.push(e.clone());
                }
                _ => {}
            }
        }

        // Overloaded functions get a `_{i}` suffix, mirroring alloy's `sol!`.
        let mut counts: HashMap<String, usize> = HashMap::new();
        for f in &functions {
            if let Some(n) = &f.name {
                *counts.entry(n.to_string()).or_default() += 1;
            }
        }
        let mut seen: HashMap<String, usize> = HashMap::new();
        let methods = functions
            .iter()
            .map(|f| {
                let base = f.name.as_ref().expect("name checked above").to_string();
                let effective = if counts.get(&base).copied().unwrap_or(0) > 1 {
                    let idx = seen.entry(base.clone()).or_insert(0);
                    let e = format!("{base}_{idx}");
                    *idx += 1;
                    e
                } else {
                    base
                };
                let method_name = if is_reserved_method(&effective) {
                    format!("{effective}_call")
                } else {
                    effective.clone()
                };
                expand_method(f, &effective, &method_name, &alloy, &aprim, &tcb)
            })
            .collect::<Result<Vec<_>>>()?;

        // Runtime bytecode — only when `#[sol(deployed_bytecode = "0x…")]` is present.
        let deployed_bytecode_tokens = match &opts.deployed_bytecode {
            None => quote!(),
            Some(bytes) => {
                let byte_vals = bytes.iter().copied();
                quote! {
                    /// The runtime bytecode of this contract, as deployed on-chain.
                    ///
                    /// Can be compared against the output of `get_contract` to verify
                    /// that the on-chain code matches the expected artifact.
                    pub static DEPLOYED_BYTECODE: #aprim::Bytes =
                        #aprim::Bytes::from_static(&[#(#byte_vals),*]);
                }
            }
        };

        // Deploy helpers — only when `#[sol(bytecode = "0x…")]` is present.
        let (deploy_tokens, deploy_instance_tokens) = match &opts.bytecode {
            None => (quote!(), quote!()),
            Some(bytes) => {
                let byte_vals = bytes.iter().copied();

                let (ctor_decls, ctor_names, ctor_values) = match &constructor {
                    None => (vec![], vec![], vec![]),
                    Some(c) => collect_params(&c.parameters, &alloy, &aprim)?,
                };

                let ctor_sig = quote!(#(, #ctor_decls)*);
                let ctor_fwd = quote!(#(, #ctor_names)*);
                let bytecode_expr = if ctor_names.is_empty() {
                    quote!(BYTECODE.clone())
                } else {
                    quote!(#aprim::Bytes::from(
                        [
                            &BYTECODE[..],
                            &#alloy::SolConstructor::abi_encode(&constructorCall {
                                #(#ctor_names: #ctor_values),*
                            })[..]
                        ].concat()
                    ))
                };

                let free_fns = quote! {
                    /// The creation bytecode of this contract.
                    pub static BYTECODE: #aprim::Bytes =
                        #aprim::Bytes::from_static(&[#(#byte_vals),*]);

                    /// Create a [`DeployBuilder`] to deploy this contract.
                    #[inline]
                    pub fn deploy_builder<P: #provider_tr>(
                        provider: P #ctor_sig
                    ) -> #deploy_builder_ty<P> {
                        Instance::<P>::deploy_builder(provider #ctor_fwd)
                    }

                    /// Deploy this contract and return a bound [`Instance`].
                    #[inline]
                    pub async fn deploy<P: #provider_tr + ::core::clone::Clone>(
                        provider: P #ctor_sig
                    ) -> #result_ty<Instance<P>> {
                        Instance::<P>::deploy(provider #ctor_fwd).await
                    }
                };

                let instance_methods = quote! {
                    impl<P: #provider_tr + ::core::clone::Clone> Instance<P> {
                        /// Create a [`DeployBuilder`] to deploy this contract.
                        #[inline]
                        pub fn deploy_builder(
                            provider: P #ctor_sig
                        ) -> #deploy_builder_ty<P> {
                            #deploy_builder_ty::new(provider, #bytecode_expr)
                        }

                        /// Deploy this contract and return a bound [`Instance`].
                        #[inline]
                        pub async fn deploy(
                            provider: P #ctor_sig
                        ) -> #result_ty<Self> {
                            let address = Self::deploy_builder(provider.clone() #ctor_fwd)
                                .deploy()
                                .await?;
                            ::core::result::Result::Ok(new(address, provider))
                        }
                    }
                };

                (free_fns, instance_methods)
            }
        };

        // Per-event filter methods, with overload suffixes mirroring alloy.
        let mut ev_counts: HashMap<String, usize> = HashMap::new();
        for e in &events {
            *ev_counts.entry(e.name.to_string()).or_default() += 1;
        }
        let mut ev_seen: HashMap<String, usize> = HashMap::new();
        let event_filter_methods = events
            .iter()
            .map(|e| {
                let base = e.name.to_string();
                let effective = if ev_counts.get(&base).copied().unwrap_or(0) > 1 {
                    let idx = ev_seen.entry(base.clone()).or_insert(0);
                    let s = format!("{base}_{idx}");
                    *idx += 1;
                    s
                } else {
                    base
                };
                let method_name = format_ident!("{}_filter", effective);
                let event_ty = format_ident!("{}", effective);
                let doc = format!("Creates an event filter for the [`{effective}`] event.");
                quote! {
                    #[doc = #doc]
                    #[allow(non_snake_case)]
                    pub fn #method_name(&self) -> #tef<P, #event_ty> {
                        self.event_filter::<#event_ty>()
                    }
                }
            })
            .collect::<Vec<_>>();

        Ok(quote! {
            #[allow(non_snake_case, clippy::pub_underscore_fields)]
            pub mod #name {
                //! Provider-bound bindings generated by `tron_sol!`.
                pub use super::#types_mod::#name::*;
                // Bring the rest of the type layer (bare structs/enums, other
                // contracts' types) into scope so custom-type parameters resolve.
                #[allow(unused_imports)]
                use super::#types_mod::*;

                #deployed_bytecode_tokens

                #deploy_tokens

                /// A provider-bound handle to this contract.
                #[derive(::core::clone::Clone)]
                pub struct Instance<P: #provider_tr> {
                    inner: #cinst<P>,
                }

                impl<P: #provider_tr> ::core::fmt::Debug for Instance<P> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        f.debug_struct(::core::stringify!(#name))
                            .field("address", &self.inner.address())
                            .finish()
                    }
                }

                /// Bind to the contract at `address` over `provider`.
                pub fn new<P: #provider_tr>(address: #taddr, provider: P) -> Instance<P> {
                    Instance { inner: #cinst::new_raw(provider, address) }
                }

                impl<P: #provider_tr> Instance<P> {
                    /// The contract address.
                    #[inline]
                    pub fn address(&self) -> #taddr {
                        self.inner.address()
                    }

                    /// Sets the contract address.
                    #[inline]
                    pub fn set_address(&mut self, address: #taddr) {
                        self.inner.set_address(address);
                    }

                    /// Return a new handle pointing at a different address.
                    #[inline]
                    pub fn at(mut self, address: #taddr) -> Self {
                        self.set_address(address);
                        self
                    }

                    /// Borrow the underlying provider.
                    #[inline]
                    pub fn provider(&self) -> &P {
                        self.inner.provider()
                    }

                    /// Build a call for any [`SolCall`] type — generic entry point
                    /// used by all typed methods.
                    #[inline]
                    pub fn call_builder<C: #alloy::SolCall>(&self, call: &C) -> #tcb<P, C> {
                        #tcb::new(self.inner.call_raw(
                            #alloy::SolCall::abi_encode(call).into()
                        ))
                    }

                    #(#methods)*
                }

                impl<P: #provider_tr> Instance<P> {
                    /// Build an event filter for any [`SolEvent`] type — generic
                    /// entry point used by all per-event filter methods.
                    #[inline]
                    pub fn event_filter<E: #alloy::SolEvent>(&self) -> #tef<P, E> {
                        #tef::new(self.inner.provider().clone(), Some(self.inner.address()))
                    }

                    #(#event_filter_methods)*
                }

                #deploy_instance_tokens
            }
        })
    }
}

/// TRON-specific options parsed from a contract's `#[sol(...)]` attributes.
#[derive(Default)]
struct ContractOpts {
    rpc: bool,
    bytecode: Option<Vec<u8>>,
    deployed_bytecode: Option<Vec<u8>>,
    /// Whether `rename`/`rename_all` is present (incompatible with `rpc`).
    rename: bool,
}

/// Read `rpc` / `bytecode` / `deployed_bytecode` / `rename(_all)` from the
/// `#[sol(...)]` attributes of an item. Other options are left for alloy's
/// `sol!` (they ride along via the forwarded token stream).
fn contract_opts(attrs: &[Attribute]) -> Result<ContractOpts> {
    let mut opts = ContractOpts::default();
    for attr in attrs {
        if !attr.path().is_ident("sol") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rpc") {
                opts.rpc = if meta.input.peek(syn::Token![=]) {
                    meta.value()?.parse::<syn::LitBool>()?.value
                } else {
                    true
                };
            } else if meta.path.is_ident("bytecode") {
                opts.bytecode = Some(parse_hex(&meta.value()?.parse::<LitStr>()?)?);
            } else if meta.path.is_ident("deployed_bytecode") {
                opts.deployed_bytecode = Some(parse_hex(&meta.value()?.parse::<LitStr>()?)?);
            } else {
                if meta.path.is_ident("rename") || meta.path.is_ident("rename_all") {
                    opts.rename = true;
                }
                // Unknown option (e.g. `all_derives`, `extra_derives(..)`,
                // `rename_all = ".."`): consume any `= value` or `(tokens)` so
                // the meta parser can continue. These ride along to alloy via
                // the forwarded stream.
                if meta.input.peek(syn::Token![=]) {
                    let _: syn::Expr = meta.value()?.parse()?;
                } else if meta.input.peek(syn::token::Paren) {
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let _: TokenStream2 = content.parse()?;
                }
            }
            Ok(())
        })?;
    }
    Ok(opts)
}

// ── token surgery: strip TRON-specific attributes before forwarding ──────────

/// Remove the TRON-specific attributes (`#[tron_sol(...)]` and the
/// `rpc`/`bytecode`/`deployed_bytecode` keys of `#[sol(...)]`) from a token
/// stream, leaving everything else — including `#[derive(...)]` and the other
/// `#[sol(...)]` keys — untouched, so the result can be fed to alloy's `sol!`.
fn strip_tron_attrs(ts: TokenStream2) -> TokenStream2 {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut out = TokenStream2::new();
    let mut i = 0;
    while i < tokens.len() {
        // Recurse into delimited groups (e.g. contract bodies).
        if let TokenTree::Group(g) = &tokens[i] {
            let mut ng = Group::new(g.delimiter(), strip_tron_attrs(g.stream()));
            ng.set_span(g.span());
            out.extend(once(TokenTree::Group(ng)));
            i += 1;
            continue;
        }

        // Attribute: `#` [`!`] `[ ... ]`
        if let TokenTree::Punct(p) = &tokens[i] {
            if p.as_char() == '#' {
                let mut j = i + 1;
                let bang = matches!(tokens.get(j), Some(TokenTree::Punct(q)) if q.as_char() == '!');
                if bang {
                    j += 1;
                }
                if let Some(TokenTree::Group(g)) = tokens.get(j) {
                    if g.delimiter() == Delimiter::Bracket {
                        // `None` => drop the whole attribute (emit nothing).
                        if let Some(inner) = rewrite_attr(g.stream()) {
                            out.extend(once(tokens[i].clone()));
                            if bang {
                                out.extend(once(tokens[i + 1].clone()));
                            }
                            let mut ng = Group::new(Delimiter::Bracket, inner);
                            ng.set_span(g.span());
                            out.extend(once(TokenTree::Group(ng)));
                        }
                        i = j + 1;
                        continue;
                    }
                }
            }
        }

        out.extend(once(tokens[i].clone()));
        i += 1;
    }
    out
}

/// Rewrite the contents of a single `[ ... ]` attribute group.
///
/// Returns `None` to drop the attribute entirely, or `Some(tokens)` with the
/// (possibly rewritten) inner tokens to keep.
fn rewrite_attr(inner: TokenStream2) -> Option<TokenStream2> {
    let toks: Vec<TokenTree> = inner.clone().into_iter().collect();
    let lead = match toks.first() {
        Some(TokenTree::Ident(id)) => id.to_string(),
        _ => return Some(inner),
    };

    match lead.as_str() {
        // `#[tron_sol(...)]` is TRON-only — drop it.
        "tron_sol" => None,
        // `#[sol(...)]` — strip the TRON-only keys, keep the rest.
        "sol" => {
            if let Some(TokenTree::Group(g)) = toks.get(1) {
                if g.delimiter() == Delimiter::Parenthesis {
                    let kept = filter_sol_meta(g.stream());
                    if kept.is_empty() {
                        return None;
                    }
                    let mut grp = Group::new(Delimiter::Parenthesis, kept);
                    grp.set_span(g.span());
                    return Some([toks[0].clone(), TokenTree::Group(grp)].into_iter().collect());
                }
            }
            Some(inner)
        }
        // Any other attribute (`derive`, `doc`, …) — keep verbatim.
        _ => Some(inner),
    }
}

/// Drop the `rpc` / `bytecode` / `deployed_bytecode` entries from the
/// comma-separated meta list inside `#[sol(...)]`, preserving the rest.
fn filter_sol_meta(stream: TokenStream2) -> TokenStream2 {
    // Split into comma-separated items (commas inside nested groups are atomic).
    let mut items: Vec<Vec<TokenTree>> = vec![Vec::new()];
    for tt in stream {
        if let TokenTree::Punct(p) = &tt {
            if p.as_char() == ',' {
                items.push(Vec::new());
                continue;
            }
        }
        items.last_mut().expect("non-empty").push(tt);
    }

    let mut out = TokenStream2::new();
    let mut first = true;
    for item in items {
        if item.is_empty() {
            continue;
        }
        let key = match item.first() {
            Some(TokenTree::Ident(id)) => id.to_string(),
            _ => String::new(),
        };
        if matches!(key.as_str(), "rpc" | "bytecode" | "deployed_bytecode") {
            continue;
        }
        if !first {
            out.extend(once(TokenTree::Punct(Punct::new(',', Spacing::Alone))));
        }
        first = false;
        out.extend(item);
    }
    out
}

/// Solidity functions whose names collide with `Instance`'s own methods get a
/// `_call` suffix, mirroring alloy's `call_builder_method_function_name`.
fn is_reserved_method(name: &str) -> bool {
    matches!(
        name,
        "new"
            | "deploy"
            | "deploy_builder"
            | "address"
            | "set_address"
            | "at"
            | "provider"
            | "call_builder"
            | "event_filter"
    )
}

/// Generates one typed instance method.
///
/// `call_base` names the `…Call` struct; `method_name` may differ when
/// `call_base` collides with a reserved `Instance` method.
fn expand_method(
    f: &ItemFunction,
    call_base: &str,
    method_name: &str,
    alloy: &TokenStream2,
    aprim: &TokenStream2,
    tcb: &TokenStream2,
) -> Result<TokenStream2> {
    let fn_ident = format_ident!("{}", method_name);
    let call_struct = format_ident!("{}Call", call_base);

    let (decls, names, values) = collect_params(&f.parameters, alloy, aprim)?;

    Ok(quote! {
        #[allow(non_snake_case, clippy::too_many_arguments)]
        pub fn #fn_ident(&self, #(#decls),*) -> #tcb<P, #call_struct> {
            self.call_builder(&#call_struct { #(#names: #values),* })
        }
    })
}

/// Maps a parameter list to `(decls, names, values)`:
/// - `decls`  — typed parameter declarations (`field: Type`)
/// - `names`  — bare field idents, for forwarding and struct init LHS
/// - `values` — value expressions (`Into::into(field)` or `field`), for struct init RHS and ABI
///   encoding
fn collect_params(
    parameters: &syn_solidity::ParameterList,
    alloy: &TokenStream2,
    aprim: &TokenStream2,
) -> Result<(Vec<TokenStream2>, Vec<Ident2>, Vec<TokenStream2>)> {
    let mut decls = Vec::new();
    let mut names = Vec::new();
    let mut values = Vec::new();
    for (i, var) in parameters.iter().enumerate() {
        // Use the inner `Ident` directly so raw identifiers (e.g. `r#type`) and
        // alloy's `self`→`this` rename are preserved without a round-trip through
        // `to_string()` which would panic on the `r#` prefix.
        let field: Ident2 = match &var.name {
            Some(n) => n.0.clone(),
            None => format_ident!("_{}", i),
        };
        match &var.ty {
            SolType::Address(..) => {
                decls.push(quote!(#field: impl ::core::convert::Into<#aprim::Address>));
                names.push(field.clone());
                values.push(quote!(::core::convert::Into::into(#field)));
            }
            other => {
                let ty = rust_ty(other, alloy, aprim)?;
                decls.push(quote!(#field: #ty));
                names.push(field.clone());
                values.push(quote!(#field));
            }
        }
    }
    Ok((decls, names, values))
}

/// Maps a Solidity type to the Rust type used in the `…Call` struct field.
///
/// Top-level `address` is handled by the caller as `impl Into<Address>`.
fn rust_ty(ty: &SolType, alloy: &TokenStream2, aprim: &TokenStream2) -> Result<TokenStream2> {
    let ts = match ty {
        SolType::Bool(_) => quote!(bool),
        // Only nested addresses reach here; top-level is handled by the caller.
        SolType::Address(..) => quote!(#aprim::Address),
        SolType::String(_) => quote!(::std::string::String),
        SolType::Bytes(_) => quote!(#aprim::Bytes),
        SolType::FixedBytes(_, n) => {
            let n = n.get() as usize;
            quote!(#aprim::FixedBytes<#n>)
        }
        SolType::Uint(_, size) => int_ty(size.map(|s| s.get()).unwrap_or(256), false, aprim),
        SolType::Int(_, size) => int_ty(size.map(|s| s.get()).unwrap_or(256), true, aprim),
        SolType::Array(arr) => {
            let inner = rust_ty(&arr.ty, alloy, aprim)?;
            match (&arr.size, arr.size_lit()) {
                (None, _) => quote!(::std::vec::Vec<#inner>),
                (Some(_), Some(lit)) => {
                    let n: usize = lit.base10_parse()?;
                    quote!([#inner; #n])
                }
                // Constant-expression sizes (e.g. `T[N]` where N is a Solidity
                // `constant`) can't be evaluated here; refuse explicitly to avoid
                // silently mismatching the `…Call` field type.
                (Some(size), None) => {
                    return Err(syn::Error::new(
                        size.span(),
                        "tron_sol! supports only integer-literal array sizes; \
                         constant expressions are not evaluated here — use a \
                         literal like `uint256[3]`",
                    ));
                }
            }
        }
        SolType::Tuple(tuple) => {
            let inners =
                tuple.types.iter().map(|t| rust_ty(t, alloy, aprim)).collect::<Result<Vec<_>>>()?;
            quote!((#(#inners,)*))
        }
        SolType::Custom(path) => {
            // Defer to `<T as SolType>::RustType` so UDVTs resolve to their
            // underlying type (e.g. `type Foo is uint256` → `U256`), matching
            // the field type `sol!` generates in the `…Call` struct.
            let id = path.last().0.clone();
            quote!(<#id as #alloy::SolType>::RustType)
        }
        SolType::Function(_) | SolType::Mapping(_) => {
            return Err(syn::Error::new(
                ty.span(),
                "`function` and `mapping` types are not supported as parameters",
            ));
        }
    };
    Ok(ts)
}

/// Maps `uintN`/`intN` to their Rust types, matching alloy's `sol!` convention:
/// 8/16/32/64/128-bit → primitives, all others → `alloy_primitives::aliases`.
fn int_ty(bits: u16, signed: bool, aprim: &TokenStream2) -> TokenStream2 {
    let primitive = matches!(bits, 8 | 16 | 32 | 64 | 128);
    if primitive {
        let id = format_ident!("{}{}", if signed { "i" } else { "u" }, bits);
        quote!(#id)
    } else {
        let id = format_ident!("{}{}", if signed { "I" } else { "U" }, bits);
        quote!(#aprim::aliases::#id)
    }
}

/// Decode a `#[sol(bytecode = "0x…")]` hex literal into raw bytes.
fn parse_hex(lit: &LitStr) -> Result<Vec<u8>> {
    let span = lit.span();
    let s = lit.value();
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(&s);
    if s.len() % 2 != 0 {
        return Err(syn::Error::new(span, "bytecode hex string has odd length"));
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = hex_nibble(pair[0], span)?;
            let lo = hex_nibble(pair[1], span)?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

fn hex_nibble(b: u8, span: Span) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(syn::Error::new(span, format!("invalid hex byte '{}'", b as char))),
    }
}
