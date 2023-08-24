use crate::{
    cargo_container::{CargoContainer, CompileOptions},
    cef::CefDist,
};
use anyhow::Result;
use maplit::hashmap;

pub async fn command(cargo: CargoContainer<'_>) -> Result<()> {
    // download cef if necessary
    let cef_dist = CefDist::from(&cargo)?;

    if !cef_dist.exists() {
        cef_dist.download().await?;
        cef_dist.extract()?;
    }

    // build cef
    let result = cargo.compile(CompileOptions {
        packages: vec!["cef-sys".into()],
        env: hashmap!["CEF_PATH".into() => cef_dist.directory()],
        ..Default::default()
    })?;

    println!("binaries:");
    result.binaries.iter().for_each(print_unit);
    println!("cdylibs:");
    result.cdylibs.iter().for_each(print_unit);

    Ok(())
}

fn print_unit(unit: &cargo::core::compiler::UnitOutput) {
    println!("unit: {:?} / path: {:?}", unit.unit, unit.path);
}
