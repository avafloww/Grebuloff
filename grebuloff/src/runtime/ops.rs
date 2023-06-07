use log::{error, info};
use deno_core::op;
use anyhow::Result;

#[op]
fn op_log(msg: &str, is_err: bool) -> Result<()> {
    if is_err {
        error!("{}", msg);
    } else {
        info!("{}", msg);
    }
    log::logger().flush();

    Ok(())
}

#[op]
fn op_print(msg: &str, is_err: bool) -> Result<()> {
    if is_err {
        error!("{}", msg);
    } else {
        info!("{}", msg);
    }
    log::logger().flush();

    Ok(())
}