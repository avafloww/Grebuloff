use anyhow::{bail, Result};
use deno_core::op;
use log::{debug, error, info, trace, warn};

#[op]
fn op_log(msg: String, level: isize) -> Result<()> {
    match level {
        -2 => trace!("{}", msg),
        -1 => debug!("{}", msg),
        0 => info!("{}", msg),
        1 => warn!("{}", msg),
        2 => error!("{}", msg),
        _ => bail!("invalid log level: {}", level),
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
