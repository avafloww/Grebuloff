use std::pin::Pin;

use anyhow::Error;
use deno_ast::{MediaType, ParseParams, SourceTextInfo};
use deno_core::{
    futures::FutureExt, ModuleSource, ModuleSourceFuture, ModuleSpecifier, ResolutionKind,
};
use log::trace;

pub struct RuntimeModuleLoader;

const FILE_EXTENSIONS: [&str; 5] = ["ts", "tsx", "d.ts", "js", "jsx"];

impl deno_core::ModuleLoader for RuntimeModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
        deno_core::resolve_import(specifier, referrer).map_err(|e| e.into())
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
        let module_specifier = module_specifier.clone();
        async move {
            let mut path = module_specifier.to_file_path().unwrap();

            if !path.is_file() {
                // If this is a js/jsx file reference, try to find the file with a ts/tsx extension.
                // This is done for runtime compilation of TypeScript files.
                match path.extension() {
                    Some(ext) if ext == "js" => {
                        let path_with_ext = path.with_extension("ts");
                        if path_with_ext.is_file() {
                            path = path_with_ext;
                        }
                    }
                    Some(ext) if ext == "jsx" => {
                        let path_with_ext = path.with_extension("tsx");
                        if path_with_ext.is_file() {
                            path = path_with_ext;
                        }
                    }
                    _ if path.is_dir() => {
                        // If this is a directory, try to find an index file.
                        for ext in FILE_EXTENSIONS.iter() {
                            let path_with_ext = path.join(format!("index.{}", ext));
                            if path_with_ext.is_file() {
                                path = path_with_ext;
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            trace!("TsModuleLoader: resolve: {:?}", path);
            // Determine what the MediaType is (this is done based on the file
            // extension) and whether transpiling is required.
            let media_type = MediaType::from_path(&path);
            let (module_type, should_transpile) = match media_type {
                MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                    (deno_core::ModuleType::JavaScript, false)
                }
                MediaType::Jsx => (deno_core::ModuleType::JavaScript, true),
                MediaType::TypeScript
                | MediaType::Mts
                | MediaType::Cts
                | MediaType::Dts
                | MediaType::Dmts
                | MediaType::Dcts
                | MediaType::Tsx => (deno_core::ModuleType::JavaScript, true),
                MediaType::Json => (deno_core::ModuleType::Json, false),
                _ => panic!("unknown media type for path: {:?}", path),
            };

            // Read the file, transpile if necessary.
            let code = std::fs::read_to_string(&path)?;
            let code = if should_transpile {
                trace!("TsModuleLoader: transpile: {:?}", path);
                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.to_string(),
                    text_info: SourceTextInfo::from_string(code),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })?;
                parsed.transpile(&Default::default())?.text
            } else {
                code
            };

            // Load and return module.
            trace!("TsModuleLoader: load: {:?}", path);
            let module = ModuleSource::new(module_type, code.into(), &module_specifier);
            Ok(module)
        }
        .boxed_local()
    }
}
