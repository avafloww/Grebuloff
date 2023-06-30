use anyhow::bail;
use ffxiv_client_structs::{
    MemberFunctionSignature, Signature, StaticAddressSignature, VTableSignature,
};
use log::{debug, info, warn};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
use windows::Win32::System::Threading::GetCurrentProcess;

static mut MODULE_START: *const u8 = std::ptr::null();
static mut MODULE_SIZE: usize = 0;

static mut TEXT_START: *const u8 = std::ptr::null();
static mut TEXT_SIZE: usize = 0;

static mut RDATA_START: *const u8 = std::ptr::null();
static mut RDATA_SIZE: usize = 0;

static mut DATA_START: *const u8 = std::ptr::null();
static mut DATA_SIZE: usize = 0;

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
        bail!("GetModuleInformation failed");
    }

    info!(
        "found module base: {:X}, size: {:X}",
        info.lpBaseOfDll as usize, info.SizeOfImage
    );

    MODULE_START = info.lpBaseOfDll as *const u8;
    MODULE_SIZE = info.SizeOfImage as usize;

    // adapted from FFXIVClientStructs.Resolver, which was adapted from Dalamud SigScanner
    let base_address = info.lpBaseOfDll as *const u8;

    // We don't want to read all of IMAGE_DOS_HEADER or IMAGE_NT_HEADER stuff so we cheat here.
    let nt_new_offset = std::ptr::read_unaligned(base_address.offset(0x3C) as *const i32);
    let nt_header = base_address.offset(nt_new_offset as isize);

    // IMAGE_NT_HEADER
    let file_header = nt_header.offset(4);
    let num_sections = std::ptr::read_unaligned(file_header.offset(6) as *const i16);

    // IMAGE_OPTIONAL_HEADER
    let optional_header = file_header.offset(20);

    let section_header = optional_header.offset(240); // IMAGE_OPTIONAL_HEADER64

    // IMAGE_SECTION_HEADER
    let mut section_cursor = section_header;
    for _ in 0..num_sections {
        let section_name = std::ptr::read_unaligned(section_cursor as *const i64);

        // .text
        match section_name {
            0x747865742E => {
                // .text
                TEXT_START = base_address.offset(std::ptr::read_unaligned(
                    section_cursor.offset(12) as *const i32
                ) as isize);
                TEXT_SIZE =
                    std::ptr::read_unaligned(section_cursor.offset(8) as *const i32) as usize;
            }
            0x617461642E => {
                // .data
                DATA_START = base_address.offset(std::ptr::read_unaligned(
                    section_cursor.offset(12) as *const i32
                ) as isize);
                DATA_SIZE =
                    std::ptr::read_unaligned(section_cursor.offset(8) as *const i32) as usize;
            }
            0x61746164722E => {
                // .rdata
                RDATA_START = base_address.offset(std::ptr::read_unaligned(
                    section_cursor.offset(12) as *const i32,
                ) as isize);
                RDATA_SIZE =
                    std::ptr::read_unaligned(section_cursor.offset(8) as *const i32) as usize;
            }
            _ => {}
        }

        section_cursor = section_cursor.offset(40); // advance by 40
    }

    info!(
        "image sections: .text {:X}(+{:X}), .data {:X}(+{:X}), .rdata {:X}(+{:X})",
        TEXT_START as usize,
        TEXT_SIZE,
        DATA_START as usize,
        DATA_SIZE,
        RDATA_START as usize,
        RDATA_SIZE
    );

    Ok(())
}

pub unsafe fn resolve_vtable(input: &VTableSignature) -> *const u8 {
    let sig_result = find_sig(TEXT_START, TEXT_SIZE, &input.signature);

    if sig_result == std::ptr::null() {
        warn!("resolve_vtable: couldn't resolve {}", input.signature);

        sig_result
    } else {
        // get the 4 bytes at input.offset bytes past the result
        let access_offset = std::ptr::read_unaligned(sig_result.offset(input.offset) as *const i32);
        let mut result =
            sig_result.offset(input.offset + 4 + access_offset as isize) as *const usize;

        if input.is_pointer {
            // dereference the pointer
            result = std::ptr::read_unaligned(result as *const *const usize);
        }

        debug!(
            "resolve_vtable: resolved {} (offset {}, is_pointer {}) - {:p}",
            input.signature, input.offset, input.is_pointer, sig_result
        );

        result as *const u8
    }
}

pub unsafe fn resolve_static_address(input: &StaticAddressSignature) -> *const u8 {
    let sig_result = find_sig(TEXT_START, TEXT_SIZE, &input.signature);

    if sig_result == std::ptr::null() {
        warn!(
            "resolve_static_address: couldn't resolve {}",
            input.signature
        );

        sig_result
    } else {
        // get the 4 bytes at input.offset bytes past the result
        let access_offset = std::ptr::read_unaligned(sig_result.offset(input.offset) as *const i32);
        let mut result =
            sig_result.offset(input.offset + 4 + access_offset as isize) as *const usize;

        if input.is_pointer {
            // dereference the pointer
            result = std::ptr::read_unaligned(result as *const *const usize);
        }

        debug!(
            "resolve_static_address: resolved {} (offset {}, is_pointer {}, access_offset {:X}, sig_result {:p}) - {:p} p, {:X} x",
            input.signature, input.offset, input.is_pointer, access_offset, sig_result, result, result as usize
        );

        result as *const u8
    }
}

pub unsafe fn resolve_member_function(input: &MemberFunctionSignature) -> *const u8 {
    let result = find_sig(TEXT_START, TEXT_SIZE, &input.signature);

    if result == std::ptr::null() {
        warn!(
            "resolve_member_function: couldn't resolve {}",
            input.signature
        );
    } else {
        debug!(
            "resolve_member_function: resolved {} - {:p}",
            input.signature, result
        );
    }

    result
}

unsafe fn find_sig(start_addr: *const u8, size: usize, sig: &Signature) -> *const u8 {
    let sig_len = sig.bytes.len();

    // we use two cursors here to handle edge cases
    // first, we iterate over the entire memory region
    'prog: for pi in 0..size {
        let prog_cursor = start_addr.add(pi);

        // next, we attempt to match the entire signature from the program cursor
        for si in 0..sig_len {
            let sig_cursor = prog_cursor.add(si);

            let valid = sig.mask[si] == 0x00 || sig.bytes[si] == *sig_cursor;
            if !valid {
                continue 'prog;
            }
        }

        // if we get here, we found the signature
        let b = *prog_cursor;
        return if b == 0xE8 || b == 0xE9 {
            // relative call
            let offset = std::ptr::read_unaligned(prog_cursor.add(1) as *const i32);
            prog_cursor.add(5).offset(offset as isize)
        } else {
            prog_cursor
        };
    }

    std::ptr::null()
}
