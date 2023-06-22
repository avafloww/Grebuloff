use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat};

#[proc_macro_attribute]
pub fn js_callable(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let orig = item.clone();

    let inputs = item.sig.inputs;
    let mut idents = vec![];
    let mut body = quote! {};

    let mut index: usize = 0;
    for input in inputs {
        if let FnArg::Typed(pat_type) = input {
            if let Pat::Ident(ident) = *pat_type.pat {
                let name = ident.ident;
                let ty = pat_type.ty;

                body = quote! {
                    #body
                    let #name = inv.args.from::<#ty>(&inv.engine, #index as usize)?;
                };

                idents.push(name);

                index += 1;
            }
        }
    }

    let callable_ident = item.sig.ident;
    let callable_name = callable_ident.to_string();
    let wrapper_ident = format_ident!("__js_callable__{}", callable_ident);

    let callable_def = quote! {
        crate::register_js_callable!(
            #callable_name,
            #wrapper_ident
        );
    };

    let wrapper = match item.sig.asyncness {
        Some(_) => {
            let async_wrapper_ident = format_ident!("__async_js_callable__{}", callable_ident);

            quote! {
                #[allow(non_snake_case)]
                async fn #async_wrapper_ident(inv: crate::runtime::engine::Invocation)
                    -> crate::runtime::engine::JsResult<crate::runtime::engine::JsValue> {
                    #body

                    crate::runtime::engine::ToJsValue::to_value(
                        #callable_ident(#(#idents),*).await,
                        &inv.engine
                    )
                }

                #[allow(non_snake_case)]
                fn #wrapper_ident(inv: crate::runtime::engine::Invocation)
                    -> crate::runtime::engine::JsResult<crate::runtime::engine::JsValue> {
                    crate::runtime::callable::execute_async(inv, #async_wrapper_ident)
                }
            }
        }
        None => quote! {
            #[allow(non_snake_case)]
            fn #wrapper_ident(inv: crate::runtime::engine::Invocation)
                -> crate::runtime::engine::JsResult<crate::runtime::engine::JsValue> {
                #body

                crate::runtime::engine::ToJsValue::to_value(
                    #callable_ident(#(#idents),*),
                    &inv.engine
                )
            }
        },
    };

    // construct the wrapper function, calling to the rust function with all of the args
    quote! {
        #wrapper
        #callable_def
        #orig
    }
    .into()
}
