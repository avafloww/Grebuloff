use std::env;
use std::path::Path;

fn gen_cef(cef_path: String) {
    use std::io::Write;
    let generated = generator(cef_path)
        .header("cef.h")
        .allowlist_type("cef_string_t")
        .allowlist_type("cef_string_userfree_t")
        .allowlist_type(".*cef_base_t")
        .allowlist_type("_cef_scheme_registrar_t")
        .allowlist_type("_cef_.*_handler_t")
        .allowlist_type("_cef_urlrequest_client_t")
        .allowlist_type("_cef_urlrequest_t")
        .allowlist_type("cef_window_handle_t")
        .allowlist_function("cef_string_.*")
        .allowlist_function("cef_execute_process")
        .allowlist_function("cef_initialize")
        .allowlist_function("cef_run_message_loop")
        .allowlist_function("cef_shutdown")
        .allowlist_function("cef_browser_host_create_browser")
        .allowlist_function("cef_urlrequest_create")
        .allowlist_function("cef_cookie_manager_get_global_manager")
        .allowlist_function("cef_.*")
        .blocklist_type("(__)?time(64)?_t")
        .blocklist_type("wchar_t")
        .blocklist_type("char16")
        .blocklist_type("u?int64")
        .blocklist_type("DWORD")
        .blocklist_type("HWND.*")
        .blocklist_type("HINSTANCE.*")
        .blocklist_type("HMENU.*")
        .blocklist_type("HICON.*")
        .blocklist_type("HCURSOR.*")
        .blocklist_type("POINT")
        .blocklist_type("MSG")
        .blocklist_type("tagMSG")
        .blocklist_type("tagPOINT")
        .blocklist_type(".*XDisplay")
        .blocklist_type("VisualID")
        .blocklist_type(".*XEvent")
        .raw_line(r#"use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::{HCURSOR, MSG}};"#)
        .raw_line("use windows::Win32::UI::WindowsAndMessaging::HMENU;")
        .raw_line("use crate::HINSTANCE;")
        .raw_line("pub type DWORD = u32;")
        .raw_line("pub type wchar_t = u16;")
        .raw_line("pub type char16 = u16;")
        .raw_line("pub type time_t = i64;")
        .raw_line("pub type int64 = ::std::os::raw::c_longlong;")
        .raw_line("pub type uint64 = ::std::os::raw::c_ulonglong;")
        .generate()
        .expect("Failed to gencef")
        .to_string();
    let new_data = generated.replace("\"stdcall\"", "\"system\"");

    // Recreate the file and dump the processed contents to it
    let mut dst = std::fs::File::create(std::path::Path::new("src").join("cef_bindings.rs"))
        .expect("Cannot create cef_bindings.rs file");
    dst.write(new_data.as_bytes())
        .expect("Cannot write cef_bindings.rs");
}

fn generator(cef_path: String) -> bindgen::Builder {
    let mut config = bindgen::CodegenConfig::FUNCTIONS;
    config.insert(bindgen::CodegenConfig::TYPES);
    bindgen::builder()
        .clang_arg(format!("-I{}", cef_path))
        .clang_arg("-fparse-all-comments")
        .clang_arg("-Wno-nonportable-include-path")
        .clang_arg("-Wno-invalid-token-paste")
        .with_codegen_config(config)
        .rustified_enum(".*")
        .derive_debug(true)
        .trust_clang_mangling(false)
        .layout_tests(false)
        .size_t_is_usize(true)
        .raw_line("#![allow(dead_code)]")
        .raw_line("#![allow(non_snake_case)]")
        .raw_line("#![allow(non_camel_case_types)]")
        .raw_line("#![allow(non_upper_case_globals)]")
        .raw_line("#![allow(unused_imports)]")
}

fn choose_source_dir() -> Option<String> {
    if let Ok(path) = env::var("CEF_ROOT") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    Some("C:\\dev\\ffxiv\\grebuloff\\hlrt\\cef-dist".to_string())
    // None
}

fn main() {
    let source_dir = choose_source_dir().expect("Failed to locate CEF lib path");
    println!("cargo:rustc-link-lib=libcef");

    // gen_cef(source_dir.clone());

    println!("Path: {:?}", source_dir);

    let release_dir = Path::new(&source_dir).join("Release");
    let resources_dir = Path::new(&source_dir).join("Resources");

    if !release_dir.exists() {
        panic!(
            "CEF Release directory ({}) does not exist",
            release_dir.to_str().unwrap_or_else(|| "")
        );
    }
    if !resources_dir.exists() {
        panic!(
            "CEF Resources directory ({}) does not exist",
            resources_dir.to_str().unwrap_or_else(|| "")
        );
    }

    if let Some(release_dir) = release_dir.to_str() {
        println!("cargo:rustc-link-search=native={}", release_dir);
    }

    // Copy the required Resources & Release contents to OUT_DIR so that a cargo run works
    let dest_path = &env::var("OUT_DIR").unwrap(); //.join("../../..");

    let opts = fs_extra::dir::CopyOptions {
        overwrite: true,
        skip_exist: false,
        buffer_size: 64000, // Default
        copy_inside: true,
        depth: 0,
        content_only: false,
    };

    let mut release_items = fs_extra::dir::get_dir_content(&release_dir).unwrap();
    let mut resources_items = fs_extra::dir::get_dir_content(&resources_dir).unwrap();

    let mut all_items = Vec::new();
    all_items.append(&mut release_items.directories);
    all_items.append(&mut release_items.files);
    all_items.append(&mut resources_items.directories);
    all_items.append(&mut resources_items.files);

    fs_extra::copy_items(&all_items, &dest_path, &opts).unwrap();
}
