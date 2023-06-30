use ffxiv_client_structs::{
    address::Addressable,
    generated::ffxiv::client::graphics::kernel::{Device, Device_Fn_Instance, SwapChain},
};
use grebuloff_macros::{function_hook, function_hook_for, vtable_functions, VTable};
use log::{debug, info, trace};
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, IDXGISwapChain_Vtbl};

#[derive(VTable)]
struct ResolvedSwapChain {
    #[vtable_base]
    base: *mut IDXGISwapChain_Vtbl,
}

vtable_functions!(impl ResolvedSwapChain {
    #[vtable_fn(8)]
    unsafe fn present(&self, sync_interval: u32, present_flags: u32);

    #[vtable_fn(13)]
    unsafe fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: u32,
        swap_chain_flags: u32,
    );
});

unsafe fn resolve_swap_chain() -> ResolvedSwapChain {
    debug!("resolving swap chain");
    let device = loop {
        let device = ffxiv_client_structs::address::get::<Device_Fn_Instance>() as *mut Device;

        if device.is_null() {
            trace!("device is null, waiting");
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        break device;
    };

    debug!("device: {:p}", device);
    let swap_chain = (*device).swap_chain;
    debug!("swap chain: {:p}", swap_chain);
    let dxgi_swap_chain = (*swap_chain).dxgiswap_chain as *mut *mut IDXGISwapChain;
    debug!("dxgi swap chain: {:p}", *dxgi_swap_chain);

    let resolved = ResolvedSwapChain {
        base: *dxgi_swap_chain as *mut IDXGISwapChain_Vtbl,
    };
    resolved
}

pub unsafe fn hook_swap_chain() {
    // #[function_hook("SwapChain::present")]
    // unsafe fn detour(&self, sync_interval: u32, present_flags: u32) {
    //     trace!(
    //         "present detour: sync_interval: {}, present_flags: {}",
    //         sync_interval,
    //         present_flags
    //     );
    //     self.original(sync_interval, present_flags)
    // }

    ///Auto-generated function hook.
    struct __FunctionHook__detour {
        hook: std::mem::ManuallyDrop<retour::RawDetour>,
        closure: *mut dyn Fn(u32, u32) -> (),
    }
    unsafe extern "C" fn detourfn(
        this: *mut IDXGISwapChain,
        sync_interval: u32,
        present_flags: u32,
    ) -> () {
        info!(
            "present detour: sync_interval: {}, present_flags: {}",
            sync_interval, present_flags
        );
        log::logger().flush();
    }
    impl __FunctionHook__detour {
        unsafe fn new(orig_address: *const usize) -> Self {
            let mut obj: std::mem::MaybeUninit<Self> = std::mem::MaybeUninit::uninit();
            let obj_ptr = obj.as_mut_ptr();
            {
                let lvl = ::log::Level::Trace;
                if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                    ::log::__private_api_log(
                        format_args!("[hook] create: SwapChain::present"),
                        lvl,
                        &(
                            "grebuloff_llrt::hooking::swapchain",
                            "grebuloff_llrt::hooking::swapchain",
                            "components\\llrt\\src\\hooking\\swapchain.rs",
                            54u32,
                        ),
                        ::log::__private_api::Option::None,
                    );
                }
            };

            // clone the object pointer so we can move it into the closure
            let obj_ptr_cloned = obj_ptr.clone();
            let closure = Box::new(move |a: u32, b: u32| {
                Self::detour(std::mem::transmute(obj_ptr_cloned), a, b)
            });

            // leak the memory so it doesn't get dropped
            // we'll manually drop it later
            let closure = Box::into_raw(closure);

            // write the closure pointer to the object
            let obj_closure_ptr = std::ptr::addr_of_mut!((*obj_ptr).closure);
            obj_closure_ptr.write(closure);

            let hook =
                retour::RawDetour::new(*orig_address as *const (), detourfn as *const ()).unwrap();
            std::ptr::addr_of_mut!((*obj_ptr).hook).write(std::mem::ManuallyDrop::new(hook));

            obj.assume_init()
        }
        unsafe extern "C" fn detour(&self, sync_interval: u32, present_flags: u32) -> () {
            trace!(
                "present detour: sync_interval: {}, present_flags: {}",
                sync_interval,
                present_flags
            );
            self.original(sync_interval, present_flags)
        }
        unsafe fn original(&self, sync_interval: u32, present_flags: u32) -> () {
            let func: unsafe extern "C" fn(u32, u32) -> () =
                std::mem::transmute(self.hook.trampoline());
            func(sync_interval, present_flags)
        }
    }
    impl FunctionHook for __FunctionHook__detour {
        unsafe fn enable(&mut self) {
            {
                let lvl = ::log::Level::Debug;
                if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                    ::log::__private_api_log(
                        format_args!("[hook] enable: SwapChain::present"),
                        lvl,
                        &(
                            "grebuloff_llrt::hooking::swapchain",
                            "grebuloff_llrt::hooking::swapchain",
                            "components\\llrt\\src\\hooking\\swapchain.rs",
                            54u32,
                        ),
                        ::log::__private_api::Option::None,
                    );
                }
            };
            self.hook.enable().unwrap();
        }
        unsafe fn disable(&mut self) {
            {
                let lvl = ::log::Level::Debug;
                if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                    ::log::__private_api_log(
                        format_args!("[hook] disable: SwapChain::present"),
                        lvl,
                        &(
                            "grebuloff_llrt::hooking::swapchain",
                            "grebuloff_llrt::hooking::swapchain",
                            "components\\llrt\\src\\hooking\\swapchain.rs",
                            54u32,
                        ),
                        ::log::__private_api::Option::None,
                    );
                }
            };
            self.hook.disable().unwrap();
        }
    }
    let swap_chain = resolve_swap_chain();

    debug!(
        "hooking swap chain present @ {:p}",
        swap_chain.address_table().present()
    );
    let mut hook = <function_hook_for!(detour)>::new(swap_chain.address_table().present());

    hook.enable();
}

trait FunctionHook {
    unsafe fn enable(&mut self);
    unsafe fn disable(&mut self);
}
