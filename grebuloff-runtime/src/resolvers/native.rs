use ffxivclientstructs::{
    MemberFunctionSignature, Signature, StaticAddressSignature, VTableSignature,
};
use log::{debug, info};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

static mut MODULE_START: *const u8 = std::ptr::null();
static mut MODULE_SIZE: usize = 0;

pub unsafe fn prepare() -> anyhow::Result<()> {
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

pub unsafe fn resolve_vtable(input: &VTableSignature) -> *const u8 {
    let mut result = find_sig(MODULE_START, MODULE_SIZE, &input.signature);

    if result == std::ptr::null() {
        debug!(
            "resolve_vtable: couldn't resolve {}",
            input.signature.string
        );
    } else {
        // get the 4 bytes at input.offset bytes past the result
        result = result.offset(input.offset);

        if input.is_pointer {
            // dereference the pointer
            result = std::ptr::read_unaligned(result as *const *const u8);
        }

        debug!(
            "resolve_vtable: resolved {} (offset {}, is_pointer {}) - {:p}",
            input.signature.string, input.offset, input.is_pointer, result
        );
    }

    result
}

pub unsafe fn resolve_static_address(input: &StaticAddressSignature) -> *const u8 {
    let mut result = find_sig(MODULE_START, MODULE_SIZE, &input.signature);

    if result == std::ptr::null() {
        debug!(
            "resolve_static_address: couldn't resolve {}",
            input.signature.string
        );
    } else {
        // get the 4 bytes at input.offset bytes past the result
        let access_offset = std::ptr::read_unaligned(result.offset(input.offset) as *const u32);
        result = result.offset(input.offset + 4 + access_offset as isize);

        if input.is_pointer {
            // dereference the pointer
            result = std::ptr::read_unaligned(result as *const *const u8);
        }

        debug!(
            "resolve_static_address: resolved {} (offset {}, is_pointer {}) - {:p}",
            input.signature.string, input.offset, input.is_pointer, result
        );
    }

    result
}

pub unsafe fn resolve_member_function(input: &MemberFunctionSignature) -> *const u8 {
    let result = find_sig(MODULE_START, MODULE_SIZE, &input.signature);

    if result == std::ptr::null() {
        debug!(
            "resolve_member_function: couldn't resolve {}",
            input.signature.string
        );
    } else {
        debug!(
            "resolve_member_function: resolved {} - {:p}",
            input.signature.string, result
        );
    }

    result
}

unsafe fn find_sig(start_addr: *const u8, size: usize, sig: &Signature) -> *const u8 {
    let sig_len = sig.bytes.len();

    let mut sig_index = 0;
    for i in 0..size {
        let ptr = start_addr.add(i);

        let sig_byte = sig.bytes[sig_index];
        let sig_mask = sig.mask[sig_index];

        let matches = if sig_mask == 0 {
            true
        } else {
            sig_byte == *ptr
        };

        if matches {
            sig_index += 1;
            if sig_index != sig_len {
                continue;
            }

            let start = ptr.sub(sig_index - 1);
            let b = *start;
            return if b == 0xE8 || b == 0xE9 {
                // relative call
                let offset = std::ptr::read_unaligned(start.add(1) as *const u32);
                start.sub(!offset as usize).add(4)
            } else {
                start
            };
        } else if sig_index > 0 {
            sig_index = 0;
        }
    }

    std::ptr::null()
}
