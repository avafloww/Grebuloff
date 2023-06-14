use std::error::Error;
use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    // Emit the instructions
    EmitBuilder::builder()
        .build_timestamp()
        .git_describe(true, true, None)
        .emit()?;
    Ok(())
}
