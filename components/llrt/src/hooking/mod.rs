use log::info;

mod swapchain;

pub unsafe fn init_hooks() {
    info!("initializing hooks");
    swapchain::hook_swap_chain();
}
