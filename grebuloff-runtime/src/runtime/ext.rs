use deno_core::Extension;
use crate::runtime::ops::*;

// Two main extensions:
// grebuloff_unprivileged: the common stuff, like logging facilities
// grebuloff_privileged: the privileged stuff, like hooking functions; only available to the core isolate

pub(crate) fn get_ext_unprivileged() -> Extension {
    Extension::builder("grebuloff_unprivileged")
        // overwrite the Deno.core.print function with one that goes to the log
        .middleware(|op| match op.name {
            "op_print" => op_print::decl(),
            _ => op
        })
        .ops(vec![
            op_log::decl(),
        ])
        .build()
}

pub(crate) fn get_ext_privileged() -> Extension {
    Extension::builder("grebuloff_privileged")
        .build()
}
