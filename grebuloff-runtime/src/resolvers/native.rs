use ffxivclientstructs::{MemberFunctionSignature, StaticAddressSignature, VTableSignature};
use log::info;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

static mut MODULE_START: *const u8 = std::ptr::null();
static mut MODULE_SIZE: usize = 0;

pub unsafe fn prepare() -> anyhow::Result<()> {
    // use the windows api to get the start address and image size of module ffxiv_dx11.exe
    // call GetModuleHandleA(NULL)

    let handle = GetModuleHandleA(None)?;
    let mut info = std::mem::zeroed::<MODULEINFO>();
    let result = GetModuleInformation(
        GetCurrentProcess(),
        handle,
        &mut info,
        std::mem::size_of::<MODULEINFO>() as u32,
    );

    if !result.as_bool() {
        return Err(anyhow::anyhow!("GetModuleInformation failed"));
    }

    info!(
        "found module base: {:X}, size: {:X}",
        info.lpBaseOfDll as usize, info.SizeOfImage
    );

    MODULE_START = info.lpBaseOfDll as *const u8;
    MODULE_SIZE = info.SizeOfImage as usize;

    Ok(())
}
pub fn resolve_vtable(input: &VTableSignature) -> Option<*const usize> {
    // TODO
    None
}

pub fn resolve_static_address(input: &StaticAddressSignature) -> Option<*const usize> {
    // TODO
    None
}

pub fn resolve_member_function(input: &MemberFunctionSignature) -> Option<*const usize> {
    // TODO
    None
}
