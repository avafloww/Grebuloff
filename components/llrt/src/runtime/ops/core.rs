use anyhow::{bail, Result};
use deno_core::{op, ops};
use log::{debug, error, info, trace, warn};

ops!(collect, [log_print]);

#[op]
fn log_print(msg: String, level: isize) -> Result<()> {
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
