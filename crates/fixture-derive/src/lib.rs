//! Attribute macro for LiteSVM's fixture harness.

use {
    proc_macro::TokenStream,
    proc_macro2::Span,
    proc_macro_crate::{crate_name, FoundCrate},
    quote::{format_ident, quote},
    syn::{parse_macro_input, FnArg, ItemFn, Pat},
};

/// Run an ordinary Rust test in an isolated Parallax program world.
///
/// A zero-argument function receives the world as an injected `ctx` binding; a
/// function declaring `(name: &mut Ctx)` keeps its explicit parameter. Either
/// form may return any type supported by Rust's test harness, including
/// `Result<(), E>`. The attribute expands to a plain `#[test]`, so filters,
/// `#[ignore]`, `#[should_panic]`, and `Result` returns all work normally.
///
/// ```rust,ignore
/// use litesvm::fixture::prelude::*;
///
/// #[parallax_test]
/// fn initializes() {
///     let authority = ctx.add(Wallet::account());
///     ctx.execute(InitializeInstruction { authority })
///         .check(Outcome::success());
/// }
/// ```
///
/// `crate::ID` is used as the program address by default. A test for another
/// program can specify `#[parallax_test(program_id = EXPR)]`. Artifact
/// discovery uses `env!("CARGO_PKG_NAME")` to prefer `target/deploy/{crate}.so`
/// in a workspace that builds several programs.
#[proc_macro_attribute]
pub fn parallax_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);

    let program_id = if attr.is_empty() {
        quote! { crate::ID }
    } else {
        let argument = parse_macro_input!(attr as syn::MetaNameValue);
        if !argument.path.is_ident("program_id") {
            return syn::Error::new_spanned(
                &argument.path,
                "expected `#[parallax_test]` or `#[parallax_test(program_id = EXPR)]`",
            )
            .to_compile_error()
            .into();
        }
        let expression = &argument.value;
        quote! { #expression }
    };

    if let Some(error) = invalid_signature(&function) {
        return error.to_compile_error().into();
    }

    let fixture = match crate_name("litesvm") {
        Ok(FoundCrate::Itself) => quote! { crate::fixture },
        Ok(FoundCrate::Name(name)) => {
            let name = format_ident!("{name}", span = Span::call_site());
            quote! { ::#name::fixture }
        }
        Err(error) => {
            return syn::Error::new(
                Span::call_site(),
                format!("could not resolve `litesvm`: {error}"),
            )
            .to_compile_error()
            .into();
        }
    };

    let attributes = &function.attrs;
    let visibility = &function.vis;
    let name = &function.sig.ident;
    let output = &function.sig.output;
    // A declared parameter keeps its name and type; a zero-argument function
    // receives the conventional `ctx` binding (call-site hygiene, so the body
    // resolves it).
    let (world_name, world_type) = match function.sig.inputs.first() {
        Some(FnArg::Typed(parameter)) => {
            let Pat::Ident(world) = &*parameter.pat else {
                unreachable!("signature validation requires an identifier pattern")
            };
            let ty = &parameter.ty;
            (world.ident.clone(), quote! { #ty })
        }
        _ => (
            syn::Ident::new("ctx", Span::call_site()),
            quote! { &mut #fixture::Ctx },
        ),
    };
    let body = &function.block;

    quote! {
        #(#attributes)*
        #[test]
        #visibility fn #name() #output {
            let mut __parallax_world = #fixture::Ctx::builder(#program_id)
                .crate_name(env!("CARGO_PKG_NAME"))
                .project_dir(env!("CARGO_MANIFEST_DIR"))
                .build()
                .unwrap_or_else(|error| ::core::panic!("{error}"));
            let #world_name: #world_type = &mut __parallax_world;
            #body
        }
    }
    .into()
}

fn invalid_signature(function: &ItemFn) -> Option<syn::Error> {
    let signature = &function.sig;
    if signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.inputs.len() > 1
    {
        return Some(signature_error(signature));
    }
    match signature.inputs.first() {
        None => None,
        Some(FnArg::Typed(parameter)) if matches!(&*parameter.pat, Pat::Ident(_)) => None,
        Some(_) => Some(signature_error(signature)),
    }
}

fn signature_error(signature: &syn::Signature) -> syn::Error {
    syn::Error::new_spanned(
        signature,
        "a #[parallax_test] function must be an ordinary function, either zero-argument (the \
         world is injected as `ctx`) or taking one world parameter: `fn name(ctx: &mut Ctx)`",
    )
}
