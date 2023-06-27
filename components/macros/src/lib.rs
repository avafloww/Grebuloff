use quote::quote;
use std::path::Path;
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
