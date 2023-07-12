use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::{Data, ItemFn, TraitItemFn, Type};

fn addr_table_name(name: String) -> Ident {
    format_ident!("{}AddressTable", name)
}

#[proc_macro_derive(VTable, attributes(vtable_base))]
pub fn derive_vtable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // find the field marked with vtable_base
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    let vtable_base = match input.data {
        Data::Struct(ref s) => {
            let vtable_base = s
                .fields
                .iter()
                .find(|f| f.attrs.iter().any(|a| a.path().is_ident("vtable_base")))
                .expect("no field marked with #[vtable_base]");

            match &vtable_base.ty {
                Type::Ptr(_) => vtable_base.ident.clone().unwrap(),
                _ => panic!("vtable_base field must be a pointer"),
            }
        }
        _ => panic!("#[derive(VTable)] can only be used on structs"),
    };

    let struct_name = input.ident;
    let addr_table_name = addr_table_name(struct_name.to_string());

    quote! {
        impl #struct_name {
            fn address_table(&self) -> #addr_table_name {
                #addr_table_name {
                    base: self.#vtable_base as *const (),
                }
            }

            fn vtable_base(&self) -> *const () {
                self.#vtable_base as *const ()
            }
        }

        struct #addr_table_name {
            base: *const (),
        }
    }
    .into()
}

#[proc_macro]
pub fn vtable_functions(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemImpl);

    let struct_name = input.self_ty;
    let addr_table_name = addr_table_name(match struct_name.as_ref() {
        Type::Path(path) => path.path.segments.last().unwrap().ident.to_string(),
        _ => panic!("#[vtable_functions] can only be used on structs"),
    });

    let mut output_addrs = quote! {};
    let mut output_impls = quote! {};

    for item in input.items {
        match item {
            syn::ImplItem::Verbatim(verbatim) => {
                let impl_fn = syn::parse2::<TraitItemFn>(verbatim)
                    .expect("vtable_functions only supports trait-like functions without bodies");

                let fn_name = impl_fn.sig.ident;
                let vtable_index = impl_fn
                    .attrs
                    .iter()
                    .find(|a| a.path().is_ident("vtable_fn"))
                    .expect("no #[vtable_fn] attribute")
                    .parse_args::<syn::LitInt>()
                    .expect("invalid #[vtable_fn] attribute")
                    .base10_parse::<usize>()
                    .expect("invalid #[vtable_fn] attribute");

                // ensure the function is marked as unsafe
                if impl_fn.sig.unsafety.is_none() {
                    panic!("#[vtable_fn] functions must be marked as unsafe");
                }

                // preserve doc comments
                let doc = impl_fn
                    .attrs
                    .iter()
                    .filter(|a| a.path().is_ident("doc"))
                    .cloned()
                    .collect::<Vec<_>>();

                // return c_void instead of the unit type, since that means we just don't know/don't care about the return type
                let return_type = match impl_fn.sig.output {
                    syn::ReturnType::Default => quote! { *mut std::ffi::c_void },
                    syn::ReturnType::Type(_, ty) => quote! { #ty },
                };

                let mut args_input = vec![];
                let mut args_typed = vec![];
                let mut args_named = vec![];

                for arg in impl_fn.sig.inputs.iter() {
                    args_input.push(quote! { #arg });

                    match arg {
                        syn::FnArg::Receiver(_) => {
                            panic!("vtable_fn functions cannot take self as an arg (you probably want to use a *const / *mut pointer)");
                        }
                        syn::FnArg::Typed(pat) => {
                            let ty = &pat.ty;
                            args_typed.push(quote! { #ty });

                            match &*pat.pat {
                                syn::Pat::Ident(ident) => {
                                    args_named.push(quote! { #ident });
                                }
                                _ => panic!("vtable_fn arguments must be named"),
                            }
                        }
                    }
                }

                output_addrs = quote! {
                    #output_addrs

                    fn #fn_name (&self) -> *const *const () {
                        unsafe { (self.base as *const usize).add(#vtable_index) as *const *const () }
                    }
                };

                output_impls = quote! {
                    #output_impls

                    #(#doc)*
                    #[doc = ""]
                    #[doc = " # Safety"]
                    #[doc = ""]
                    #[doc = " This function is unsafe because it calls a C++ virtual function by address."]
                    unsafe fn #fn_name (&self, #(#args_input),*) -> #return_type {
                        let address = self.address_table().#fn_name();
                        let func: extern "C" fn(#(#args_typed),*) -> #return_type = std::mem::transmute(address);
                        func(#(#args_named),*)
                    }
                };
            }
            _ => panic!("vtable_functions only supports trait-like functions without bodies"),
        }
    }

    quote! {
        impl #addr_table_name {
            #output_addrs
        }

        impl #struct_name {
            #output_impls
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn function_hook(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let impl_fn = syn::parse_macro_input!(input as ItemFn);

    let fn_name = impl_fn.sig.ident;
    let hook_name = format_ident!("__hook__{}", fn_name);
    let detour_name = format_ident!("__detour__{}", fn_name);

    // ensure the function is marked as unsafe
    if impl_fn.sig.unsafety.is_none() {
        panic!("function hooks must be marked as unsafe");
    }

    // preserve doc comments
    let doc = impl_fn
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .cloned()
        .collect::<Vec<_>>();

    let return_type = match impl_fn.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let mut args_input = vec![];
    let mut args_named = vec![];
    let mut fn_type_args = vec![];

    for arg in impl_fn.sig.inputs.iter() {
        args_input.push(quote! { #arg });

        match arg {
            syn::FnArg::Typed(pat) => {
                let ty = &pat.ty;
                fn_type_args.push(quote! { #ty });

                match &*pat.pat {
                    syn::Pat::Ident(ident) => {
                        args_named.push(quote! { #ident });
                    }
                    _ => panic!("function_hook arguments must be named"),
                }
            }
            _ => {}
        }
    }

    let fn_type = quote! {
        fn(#(#fn_type_args),*) -> #return_type
    };
    let fn_body = impl_fn.block.stmts;

    // preserve calling convention, if specified on the function, otherwise default to C
    let abi = impl_fn
        .sig
        .abi
        .as_ref()
        .map(|abi| quote! { #abi })
        .unwrap_or_else(|| quote! { extern "C" });

    quote! {
        #[doc = "Auto-generated function hook."]
        #(#doc)*
        #[allow(non_upper_case_globals)]
        static_detour! {
            static #hook_name: unsafe #abi #fn_type;
        }

        #[doc = "Auto-generated function hook."]
        #[doc = ""]
        #(#doc)*
        #[doc = "# Safety"]
        #[doc = "This function is unsafe and should be treated as such, despite its lack of an `unsafe` keyword."]
        #[doc = "This function should not be called outside of hooked native game code."]
        #[allow(non_snake_case)]
        fn #detour_name (#(#args_input),*) -> #return_type {
            // wrap everything in an unsafe block here
            // we can't easily pass an unsafe function to initialize otherwise, since we would
            // have to wrap it in a closure, which would require knowing the closure signature,
            // which we don't know at compile time/from a proc macro, at least not easily
            let original = &#hook_name;
            unsafe {
                #(#fn_body)*
            }
        }
    }
    .into()
}

#[proc_macro]
pub fn __fn_hook_symbol(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let fn_name = syn::parse_macro_input!(input as Ident);

    let sym = format_ident!("__hook__{}", fn_name);

    quote! { #sym }.into()
}

#[proc_macro]
pub fn __fn_detour_symbol(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let fn_name = syn::parse_macro_input!(input as Ident);

    let sym = format_ident!("__detour__{}", fn_name);

    quote! { #sym }.into()
}
