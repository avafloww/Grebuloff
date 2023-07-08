use std::{error::Error, process::Command};

const LIBHLRT_BASE: &str = "../build/libhlrt/dist";

struct Meta;
impl Meta {
    fn version() {
        let out = Command::new("git")
            .arg("describe")
            .arg("--always")
            .arg("--dirty")
            .output()
            .unwrap();
        println!(
            "cargo:rustc-env=GIT_DESCRIBE={}",
            String::from_utf8(out.stdout).unwrap()
        );
    }

    fn timestamp() {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    Meta::version();
    Meta::timestamp();

    Ok(())
}
