#![feature(string_leak)]

// use deno_core::snapshot_util::{create_snapshot, CreateSnapshotOptions};
// use deno_core::{include_js_files, Extension, ExtensionFileSource, ExtensionFileSourceCode};
use std::error::Error;
use std::path::Path;
use vergen::EmitBuilder;
use walkdir::WalkDir;

const LIBHLRT_BASE: &str = "../build/libhlrt/dist";

fn emit_build_meta() -> Result<(), Box<dyn Error>> {
    EmitBuilder::builder()
        .build_timestamp()
        .git_describe(true, true, None)
        .emit()?;

    Ok(())
}

fn build_libhlrt_snapshot() -> Result<(), Box<dyn Error>> {
    // let mut files = Vec::new();
    //
    // let base_path = Path::new(&LIBHLRT_BASE).canonicalize()?;
    // for entry in WalkDir::new(&base_path).into_iter().filter_map(|e| e.ok()) {
    //     let path = entry.path();
    //     match path.extension() {
    //         Some(ext) if ext == "js" => {
    //             println!("cargo:rerun-if-changed={}", path.display());
    //
    //             // get the absolute path to the file
    //             let path = path.canonicalize()?;
    //
    //             // get the path to the file, relative to the base path
    //             let rel_path = path.strip_prefix(&base_path).unwrap();
    //
    //             // normalize rel_path to use forward slashes, strip leading slash, and strip extension
    //             let rel_path = rel_path
    //                 .to_str()
    //                 .unwrap()
    //                 .replace("\\", "/")
    //                 .leak() // doesn't really matter, this is a build script
    //                 .trim_start_matches('/')
    //                 .trim_end_matches(".js");
    //
    //             let rel_path = format!("ext:grebuloff/{}", rel_path);
    //
    //             let js_file = ExtensionFileSource {
    //                 specifier: rel_path.leak(),
    //                 code: ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(path),
    //             };
    //
    //             files.push(js_file);
    //         }
    //         _ => continue,
    //     }
    // }
    // let files = include_js_files!(grebuloff
    //     "../build/libhlrt/dist/index.js",
    //     // "../build/libhlrt/dist/console.js",
    // );
    //
    // // build the extension
    // let ext = Extension::builder("grebuloff").esm(files).build();
    //
    // // snapshot time!
    // let out_path =
    //     Path::new(std::env::var("OUT_DIR").unwrap().as_str()).join("libhlrt_snapshot.bin");
    //
    // create_snapshot(CreateSnapshotOptions {
    //     cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
    //     snapshot_path: out_path,
    //     startup_snapshot: None,
    //     extensions: vec![ext],
    //     compression_cb: None,
    //     snapshot_module_load_cb: None,
    // });

    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    // Rerun if libhlrt changes, so we can regenerate the snapshot
    println!("cargo:rerun-if-changed={}", LIBHLRT_BASE);
    build_libhlrt_snapshot()?;

    // Emit build metadata
    emit_build_meta()?;

    Ok(())
}
