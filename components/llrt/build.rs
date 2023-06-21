use std::error::Error;
use vergen::EmitBuilder;

const LIBHLRT_BASE: &str = "../build/libhlrt/dist";

fn emit_build_meta() -> Result<(), Box<dyn Error>> {
    EmitBuilder::builder()
        .build_timestamp()
        .git_describe(true, true, None)
        .emit()?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Rerun if libhlrt changes, so we can regenerate the snapshot
    println!("cargo:rerun-if-changed={}", LIBHLRT_BASE);

    // Emit build metadata
    emit_build_meta()?;

    Ok(())
}
