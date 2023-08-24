use std::{env, path::PathBuf};

fn main() {
    let cef_path = env::var("CEF_PATH").expect("CEF_PATH not set");

    println!(
        "cargo:rustc-link-search={}",
        PathBuf::from(cef_path).join("Release").display()
    );

    println!("cargo:rustc-link-lib=libcef");
    println!("cargo:rustc-link-lib=cef_sandbox");
    println!("cargo:rustc-link-lib=delayimp");

    println!("cargo:rerun-if-changed=cef.h");
    println!("cargo:rerun-if-changed=build.rs");
}
