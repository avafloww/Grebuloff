use deno_core::OpDecl;

mod core;

pub fn collect() -> Vec<OpDecl> {
    let mut ops = Vec::new();

    ops.extend(core::collect());

    ops
}
