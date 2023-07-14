use std::{error::Error, process::Command};

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

struct Build;
impl Build {
    fn build_hlrt() {
        // only run `pnpm install` if hlrt/node_modules is absent
        if !std::path::Path::new("hlrt").join("node_modules").exists() {
            Command::new("cmd")
                .arg("/C")
                .arg("pnpm")
                .arg("install")
                .current_dir("hlrt")
                .spawn()
                .expect("failed to run `pnpm install` for HLRT");
        }

        Command::new("cmd")
            .arg("/C")
            .arg("pnpm")
            .arg("maybe-build:js")
            .current_dir("hlrt")
            .spawn()
            .expect("failed to run `pnpm maybe-build:js` for HLRT");
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    Meta::version();
    Meta::timestamp();

    Build::build_hlrt();

    Ok(())
}
