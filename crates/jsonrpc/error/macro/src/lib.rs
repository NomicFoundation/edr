//! `#[rpc_error]` or `#[rpc_error(tag = "...")]` — attribute macro that
//! wraps a struct with `#[derive(serde::Serialize)]`, while enforcing:
//!
//!   1. The struct must have named fields. Tuple structs, unit structs, enums,
//!      and unions are rejected.
//!   2. Any `#[serde(...)]` attribute on the struct or its fields must come
//!      from a closed whitelist of attributes that preserve the "serializes as
//!      a JSON object with named keys" invariant. `#[cfg_attr(...,
//!      serde(...))]` is recursed into, so conditional serde attrs are policed
//!      just like unconditional ones.
//!
//! ## Tag handling
//!
//! If the attribute is written as `#[rpc_error(tag = "my-tag")]`, the
//! macro emits an `impl RpcStructuredErrorTag` with the given tag.
//!
//! If written as bare `#[rpc_error]`, no tag impl is emitted — the user
//! is expected to write their own. This is useful when the tag must be
//! computed from other sources, shared across multiple types, or when
//! the user wants full control over the trait impl.
//!
//! Whitelisted serde attributes:
//!   - On the struct: `rename_all`
//!   - On a field:    `rename`, `rename_all`, `skip_serializing_if`,
//!     `skip_serializing`, `skip`, `serialize_with`, `default`, `borrow`,
//!     `with` (with field-level semantics — does NOT change outer shape)
//!
//! Notably banned:
//!   - `transparent`     — would make the struct serialize as its single field
//!   - `flatten`         — would inline keys from a sub-object
//!   - `tag`/`untagged`  — enum-only, but explicitly named
//!   - `into`/`from`/`try_from` — re-routes serialization through another type
//!   - `serialize_with` on the STRUCT (vs. on a field) — replaces whole impl
//!   - `remote`          — re-routes serialization
//!   - any unknown `#[serde(...)]` key (defaults to denied)

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute, Data, DeriveInput, Fields, LitStr, Token,
};

/// Whitelist of serde attribute keys allowed at the **struct** level.
const STRUCT_LEVEL_WHITELIST: &[&str] = &["rename_all"];

/// Whitelist of serde attribute keys allowed at the **field** level.
/// These all preserve the outer object shape; they only affect a single
/// field's key or value.
const FIELD_LEVEL_WHITELIST: &[&str] = &[
    "rename",
    "rename_all",
    "skip_serializing_if",
    "skip_serializing",
    "skip",
    "serialize_with",
    "default",
    "borrow",
    "with",
];

// ---------------------------------------------------------------------
// Argument parsing for `#[rpc_error(tag = "...")]`
// ---------------------------------------------------------------------

struct RpcErrorArgs {
    tag: Option<String>,
}

impl Parse for RpcErrorArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut tag = None;
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            if key == "tag" {
                if tag.is_some() {
                    return Err(syn::Error::new(key.span(), "duplicate `tag` argument"));
                }
                tag = Some(value.value());
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unknown argument `{key}`; expected `tag`"),
                ));
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(RpcErrorArgs { tag })
    }
}

// ---------------------------------------------------------------------
// Attribute validation
// ---------------------------------------------------------------------

/// Walk a list of attributes, find every `#[serde(...)]` (including
/// inside `#[cfg_attr(..., serde(...))]`), and validate each against
/// the whitelist. Returns all violations.
///
/// Other attribute kinds (`doc`, `cfg`, `allow`, custom proc-macro attrs,
/// etc.) are left alone — they don't affect serialization shape.
fn validate_serde_attrs(attrs: &[Attribute], whitelist: &[&str], scope: &str) -> Vec<syn::Error> {
    let mut errors = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("serde") {
            check_serde_meta(attr, whitelist, scope, &mut errors);
        } else if attr.path().is_ident("cfg_attr") {
            // `#[cfg_attr(predicate, attr1, attr2, ...)]` — we don't care
            // about the predicate (it can be anything), but the conditionally-
            // applied attrs need the same scrutiny as if they were applied
            // unconditionally. Otherwise `cfg_attr(any(), serde(transparent))`
            // would slip past us.
            let parsed = attr.parse_args_with(
                syn::punctuated::Punctuated::<CfgAttrPart, Token![,]>::parse_terminated,
            );
            match parsed {
                Ok(parts) => {
                    // First entry is the predicate; remaining are conditional attrs.
                    for part in parts.iter().skip(1) {
                        if let CfgAttrPart::SerdeAttr(inner) = part {
                            check_serde_meta_from_tokens(
                                inner,
                                whitelist,
                                scope,
                                &mut errors,
                                attr,
                            );
                        }
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        // All other attribute kinds: ignored (doc, cfg, allow, etc.).
    }
    errors
}

/// Walks the inside of a `#[serde(...)]` attribute, checking every key.
fn check_serde_meta(
    attr: &Attribute,
    whitelist: &[&str],
    scope: &str,
    errors: &mut Vec<syn::Error>,
) {
    let parse_result = attr.parse_nested_meta(|meta| {
        let key_ident = if let Some(i) = meta.path.get_ident() {
            i.clone()
        } else {
                errors.push(syn::Error::new_spanned(
                    &meta.path,
                    format!("complex serde attribute paths are not allowed at the {scope} level"),
                ));
                let _ = meta.value().and_then(syn::parse::ParseBuffer::parse::<proc_macro2::TokenStream>);
                return Ok(());
            };
        let key_str = key_ident.to_string();
        if !whitelist.contains(&key_str.as_str()) {
            errors.push(syn::Error::new(
                key_ident.span(),
                format!(
                    "`#[serde({key_str})]` is not allowed at the {scope} level of an `#[rpc_error]` struct \
                     because it could change or replace the serialized shape. \
                     Allowed at this level: {whitelist:?}",
                ),
            ));
        }
        if meta.input.peek(Token![=]) {
            let _ = meta.value().and_then(syn::parse::ParseBuffer::parse::<proc_macro2::TokenStream>);
        } else if meta.input.peek(syn::token::Paren) {
            let _ = meta.input.parse::<proc_macro2::TokenStream>();
        }
        Ok(())
    });
    if let Err(e) = parse_result {
        errors.push(e);
    }
}

/// One part of a `cfg_attr(predicate, attr1, attr2, ...)`. We only care
/// about distinguishing "this is a `serde(...)` payload" from everything
/// else — predicates and unrelated attrs are kept as opaque token streams.
enum CfgAttrPart {
    SerdeAttr(proc_macro2::TokenStream),
    Other,
}

impl Parse for CfgAttrPart {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        // Peek for an ident followed by `(...)`.
        if input.peek(syn::Ident) && input.peek2(syn::token::Paren) {
            let ident: syn::Ident = input.fork().parse()?;
            if ident == "serde" {
                let _ident: syn::Ident = input.parse()?;
                let content;
                syn::parenthesized!(content in input);
                let tokens: proc_macro2::TokenStream = content.parse()?;
                return Ok(CfgAttrPart::SerdeAttr(tokens));
            }
        }
        // Anything else: consume until next comma at the top level.
        // Easiest correct way: parse any `Meta`.
        let _: syn::Meta = input.parse()?;
        Ok(CfgAttrPart::Other)
    }
}

/// Validate a `serde(...)` payload that came from inside `cfg_attr`.
/// We don't have a real `Attribute`, so we synthesize one for
/// `parse_nested_meta`.
fn check_serde_meta_from_tokens(
    tokens: &proc_macro2::TokenStream,
    whitelist: &[&str],
    scope: &str,
    errors: &mut Vec<syn::Error>,
    span_anchor: &Attribute,
) {
    // Synthesize `#[serde(<tokens>)]` and parse it into an Attribute,
    // then reuse check_serde_meta. Token-tree round-trip preserves spans
    // for the inner content well enough to point at the right key.
    let synthesized: TokenStream2 = quote! { #[serde(#tokens)] };
    let parsed: syn::Result<Attribute> = syn::parse2::<syn::ItemStruct>(quote! {
        #synthesized
        struct __X {}
    })
    .and_then(|s| {
        s.attrs.into_iter().next().ok_or_else(|| {
            syn::Error::new_spanned(
                span_anchor,
                "internal: failed to re-parse cfg_attr serde payload",
            )
        })
    });
    match parsed {
        Ok(attr) => check_serde_meta(&attr, whitelist, scope, errors),
        Err(e) => errors.push(e),
    }
}

// ---------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------

#[proc_macro_attribute]
pub fn rpc_error(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as RpcErrorArgs);
    let mut input = parse_macro_input!(item as DeriveInput);

    let mut errors: Vec<syn::Error> = Vec::new();

    // ---- 1. Shape check -----------------------------------------------
    let fields_named = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(_) => true,
            Fields::Unit => {
                errors.push(syn::Error::new_spanned(
                    &input.ident,
                    "`#[rpc_error]` requires a struct with named fields. \
                     Unit structs serialize to `null`, not a JSON object. \
                     Change `struct Foo;` to `struct Foo {}` for an empty payload.",
                ));
                false
            }
            Fields::Unnamed(_) => {
                errors.push(syn::Error::new_spanned(
                    &input.ident,
                    "`#[rpc_error]` requires a struct with named fields. \
                     Tuple structs serialize to a JSON array.",
                ));
                false
            }
        },
        Data::Enum(e) => {
            errors.push(syn::Error::new_spanned(
                e.enum_token,
                "`#[rpc_error]` requires a struct with named fields, not an enum.",
            ));
            false
        }
        Data::Union(u) => {
            errors.push(syn::Error::new_spanned(
                u.union_token,
                "`#[rpc_error]` cannot be used on unions.",
            ));
            false
        }
    };

    // ---- 2. serde attribute whitelist check ---------------------------
    errors.extend(validate_serde_attrs(
        &input.attrs,
        STRUCT_LEVEL_WHITELIST,
        "struct",
    ));
    if fields_named
        && let Data::Struct(s) = &input.data
        && let Fields::Named(named) = &s.fields
    {
        for field in &named.named {
            errors.extend(validate_serde_attrs(
                &field.attrs,
                FIELD_LEVEL_WHITELIST,
                "field",
            ));
        }
    }

    // ---- 3. If any errors, emit them all and stop ---------------------
    if !errors.is_empty() {
        let mut combined = errors.into_iter();
        let first = combined.next().unwrap();
        let folded = combined.fold(first, |mut acc, e| {
            acc.combine(e);
            acc
        });
        return folded.to_compile_error().into();
    }

    // ---- 4. Add #[derive(serde::Serialize)] to the struct's attrs -----
    // We *prepend* a derive attribute. The user's original attributes
    // (including any whitelisted #[serde(...)] ones) remain in place,
    // and serde's derive will see them.
    let derive_attr: Attribute = syn::parse_quote!(#[derive(::serde::Serialize)]);
    input.attrs.insert(0, derive_attr);

    // ---- 5. Emit the modified struct. Optionally emit the tag impl. ---
    // If the user wrote `#[rpc_error(tag = "...")]`, we generate the trait
    // impl. If they wrote bare `#[rpc_error]`, we leave the impl out so
    // they can provide their own — e.g. a context-dependent tag computed
    // from other associated constants, or a tag that varies by build.
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let tag_impl: TokenStream2 = match args.tag {
        Some(tag) => quote! {
            impl #impl_generics ::edr_jsonrpc_error_structured::RpcStructuredErrorTag
                for #ident #ty_generics #where_clause
            {
                const ERROR_TAG: &'static str = #tag;
            }
        },
        None => TokenStream2::new(),
    };

    let output: TokenStream2 = quote! {
        #input
        #tag_impl
    };

    output.into()
}
