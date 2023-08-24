use std::{fmt, sync::OnceLock};

use anyhow::Result;
use log::{debug, info};
use retour::StaticDetour;

mod framework;
mod swapchain;
mod wndproc;

static mut HOOK_MANAGER: OnceLock<HookManager> = OnceLock::new();

pub struct HookManager {
    pub hooks: Vec<FunctionHook>,
}

impl HookManager {
    pub fn instance() -> &'static Self {
        unsafe { HOOK_MANAGER.get().unwrap() }
    }

    pub fn dump_hooks(&self) {
        info!("dump_hooks: {} total hooks registered", self.hooks.len());
        for hook in &self.hooks {
            info!("dump_hooks: {:?}", hook);
        }
    }
}

/// A lightweight pointer to a function hook.
#[derive(Copy, Clone)]
pub struct FunctionHook {
    pub name: &'static str,
    pub address: usize,
    detour: &'static dyn FunctionHookOps,
}

pub trait FunctionHookOps {
    unsafe fn enable(&self) -> Result<()>;
    unsafe fn disable(&self) -> Result<()>;
    fn is_enabled(&self) -> bool;
}

impl<F: retour::Function> FunctionHookOps for StaticDetour<F> {
    unsafe fn enable(&self) -> Result<()> {
        Ok(self.enable()?)
    }

    unsafe fn disable(&self) -> Result<()> {
        Ok(self.disable()?)
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }
}

impl fmt::Debug for FunctionHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} @ 0x{:X} (enabled: {})",
            self.name,
            self.address,
            self.detour.is_enabled()
        )
    }
}

impl FunctionHook {
    pub fn new(name: &'static str, address: usize, detour: &'static dyn FunctionHookOps) -> Self {
        let hook = Self {
            name,
            address,
            detour,
        };

        debug!("[hook] register: {}", hook.name);
        unsafe { HOOK_MANAGER.get_mut().unwrap().hooks.push(hook) };

        hook
    }

    pub unsafe fn enable(&self) -> Result<()> {
        debug!("[hook] enable: {}", self.name);
        self.detour.enable()
    }

    pub unsafe fn disable(&self) -> Result<()> {
        debug!("[hook] disable: {}", self.name);
        self.detour.disable()
    }

    pub fn is_enabled(&self) -> bool {
        self.detour.is_enabled()
    }
}

pub unsafe fn init_early_hooks() -> Result<()> {
    info!("initializing hook manager");
    HOOK_MANAGER.get_or_init(|| HookManager { hooks: Vec::new() });

    info!("initializing early hooks");
    framework::hook_framework()?;

    Ok(())
}

pub unsafe fn init_hooks() -> Result<()> {
    info!("initializing hooks");

    swapchain::hook_swap_chain()?;
    // wndproc::hook_wndproc()?;

    Ok(())
}

macro_rules! create_function_hook {
    ($detour:ident, $target:expr) => {{
        ::grebuloff_macros::__fn_hook_symbol!($detour).initialize(
            ::std::mem::transmute($target),
            ::grebuloff_macros::__fn_detour_symbol!($detour),
        )?;
        crate::hooking::FunctionHook::new(
            concat!(module_path!(), "::", stringify!($detour)),
            $target as usize,
            &::grebuloff_macros::__fn_hook_symbol!($detour),
        )
    }};
}
pub(crate) use create_function_hook;
