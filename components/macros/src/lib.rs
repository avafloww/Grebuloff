use proc_macro2::Ident;
use quote::{format_ident, quote};
use std::path::Path;
use syn::{Data, ItemFn, TraitItemFn, Type};
use walkdir::WalkDir;

#[proc_macro]
pub fn libhlrt_js_files(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let libhlrt_dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../libhlrt/dist/");

    let walker = WalkDir::new(libhlrt_dist.clone())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().unwrap_or_default() == "js");

    let mut files = Vec::new();

    for entry in walker {
        let full_path = entry.path();
        let specifier = full_path.strip_prefix(libhlrt_dist.clone()).unwrap();
        let specifier = format!("libhlrt/{}", specifier.to_str().unwrap());
        let full_path = full_path.to_str().unwrap();

        files.push(quote! {
            deno_core::ExtensionFileSource {
                specifier: #specifier,
                code: deno_core::ExtensionFileSourceCode::IncludedInBinary(include_str!(#full_path)),
            },
        });
    }

    quote! {
        vec![
            #(#files)*
        ]
    }
    .into()
}

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
                    base: self.#vtable_base as *const usize,
                }
            }

            fn vtable_base(&self) -> *const usize {
                self.#vtable_base as *const usize
            }
        }

        struct #addr_table_name {
            base: *const usize,
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
                            // todo: receiver needs more work probably
                            args_typed.push(quote! { *mut _ });
                            args_named.push(quote! { self.vtable_base() as *mut std::ffi::c_void });
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

                    fn #fn_name (&self) -> *const usize {
                        unsafe { self.base.add(#vtable_index) }
                    }
                };

                output_impls = quote! {
                    #output_impls

                    #(#doc)*
                    #[doc = ""]
                    #[doc = " # Safety"]
                    #[doc = ""]
                    #[doc = " This function is unsafe because it calls a C++ virtual function by address."]
                    unsafe fn #fn_name (#(#args_input),*) -> #return_type {
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

/*
usage:

register_function_hook!(resolve_swap_chain().address_table().present(), detour);

#[function_hook]
fn detour(&self, a: u32, b: u32) -> f64 {
    println!("detour: {}, {}", a, b);
    self.original(a, b) + 1
}


becomes:

struct PresentDetour {
    hook: MaybeUninit<GenericDetour<fn(u32, u32) -> f64>>,
}

impl PresentDetour {
    type DetourFn = fn(u32, u32) -> ();

    unsafe fn new() -> Self {}

    unsafe extern "C" fn detour(&self, sync_interval: u32, present_flags: u32) -> f64 {
        println!("detour: {}, {}", a, b);
        self.original(a, b) + 1
    }

    unsafe fn original(&self, sync_interval: u32, present_flags: u32) -> f64 {
        self.hook.assume_init()(sync_interval, present_flags)
    }
}

impl FunctionHook for PresentDetour {
    unsafe fn enable(&mut self) {
        self.detour.enable().unwrap();
    }

    unsafe fn disable(&mut self) {
        self.detour.disable().unwrap();
    }
}

*/

fn function_hook_name(name: String) -> Ident {
    format_ident!("__FunctionHook__{}", name)
}

#[proc_macro_attribute]
pub fn function_hook(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let impl_fn = syn::parse_macro_input!(input as ItemFn);

    // attr can optionally be a literal string, which is used as a hook description in logs
    let fn_description = syn::parse_macro_input!(attr as Option<syn::LitStr>);

    let fn_name = impl_fn.sig.ident;
    let struct_name = function_hook_name(fn_name.to_string());

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

    let log_name = fn_description
        .as_ref()
        .map(|s| s.value())
        .unwrap_or_else(|| fn_name.to_string());
    let create_msg = format!("[hook] create: {}", log_name);
    let enable_msg = format!("[hook] enable: {}", log_name);
    let disable_msg = format!("[hook] disable: {}", log_name);

    quote! {
        #[doc = "Auto-generated function hook."]
        #(#doc)*
        struct #struct_name {
            hook: retour::GenericDetour<#fn_type>,
        }

        impl #struct_name {
            unsafe fn new(orig_address: *const usize) -> Self {
                let mut obj: std::mem::MaybeUninit<Self> = std::mem::MaybeUninit::uninit();
                let obj_ptr = obj.as_mut_ptr();

                log::trace!(#create_msg);
                let hook = retour::GenericDetour::<#fn_type>::new(
                    std::mem::transmute(orig_address),
                    std::mem::transmute(std::ptr::addr_of_mut!((*obj_ptr).detour)),
                )
                .unwrap();
                std::ptr::addr_of_mut!((*obj_ptr).hook).write(hook);

                obj.assume_init()
            }

            unsafe extern "C" fn detour(#(#args_input),*) -> #return_type {
                #(#fn_body)*
            }

            unsafe fn original(#(#args_input),*) -> #return_type {
                self.hook.call(#(#args_named),*)
            }
        }

        impl FunctionHook for #struct_name {
            unsafe fn enable(&mut self) {
                log::debug!(#enable_msg);
                self.hook.enable().unwrap();
            }

            unsafe fn disable(&mut self) {
                log::debug!(#disable_msg);
                self.hook.disable().unwrap();
            }
        }
    }
    .into()
}

/*
usage:
function_hook_for!(detour)::new(resolve_swap_chain().address_table().present());


result:
__FunctionHook__detour::new(resolve_swap_chain().address_table().present());
 */
#[proc_macro]
pub fn function_hook_for(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let fn_name = syn::parse_macro_input!(input as Ident);
    let struct_name = function_hook_name(fn_name.to_string());

    quote! {
        #struct_name
    }
    .into()
}
