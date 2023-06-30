use std::{env, path::Path};

const CEF_VERSION: &str = "114.2.11+g87c8807+chromium-114.0.5735.134";

/// Returns the file name (without extension) of the CEF archive for the current platform.
fn check_platform() -> String {
    let triple = env::var("TARGET").unwrap();
    let triple = triple.split("-");
    match triple.collect::<Vec<_>>().as_slice() {
        ["x86_64", "pc", "windows", "msvc"] => {
            println!("cargo:rustc-link-lib=libcef");
            format!("cef_binary_{}_windows64_minimal", CEF_VERSION)
        }
        _ => panic!("Unsupported target triple: {}", env::var("TARGET").unwrap()),
    }
}

fn download_cef(cef_binary: String) -> Option<String> {
    let _cef_archive_url = format!("https://cef-builds.spotifycdn.com/{}.tar.bz2", cef_binary);
    panic!("CEF download not yet supported");
}

fn find_cef() -> Option<String> {
    let cef_binary = check_platform();

    // Check if the CEF_ROOT environment variable is set
    // If it is, use that path, as long as it exists
    if let Ok(path) = env::var("CEF_ROOT") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    // Check to see if we have an extracted CEF binary in the manifest directory or in the parent directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir);

    if Path::join(manifest_dir, &cef_binary).exists() {
        return Some(manifest_dir.join(&cef_binary).to_str()?.to_owned());
    } else if Path::join(manifest_dir.parent()?, &cef_binary).exists() {
        return Some(
            manifest_dir
                .parent()?
                .join(&cef_binary)
                .to_str()?
                .to_owned(),
        );
    }

    // Otherwise, try to download it
    download_cef(cef_binary)
}

fn main() {
    let source_dir = find_cef().expect("Failed to locate CEF lib path");

    println!("CEF path: {:?}", source_dir);

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
    let dest_path = Path::new(&env::var("OUT_DIR").unwrap()).join("../../..");

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
