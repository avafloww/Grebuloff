#![allow(unsafe_code)]

use clap::Parser;
use dll_syringe::{process::OwnedProcess, Syringe};
use std::os::windows::io::FromRawHandle;
use std::path::PathBuf;
use std::ptr::addr_of_mut;
use sysinfo::{PidExt, ProcessExt, SystemExt};
use windows::Win32::Foundation::{CloseHandle, LUID};
use windows::Win32::Security::Authorization::{
    GetSecurityInfo, SetSecurityInfo, GRANT_ACCESS, SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, PrivilegeCheck, ACE_FLAGS, ACL,
    DACL_SECURITY_INFORMATION, LUID_AND_ATTRIBUTES, PRIVILEGE_SET, SECURITY_DESCRIPTOR,
    SE_DEBUG_NAME, SE_PRIVILEGE_ENABLED, SE_PRIVILEGE_REMOVED, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken, CREATE_SUSPENDED};
use windows::{
    core::{HSTRING, PWSTR},
    imp::GetLastError,
    Win32::{
        Foundation::{BOOL, HANDLE},
        Security::{
            Authorization::{BuildExplicitAccessWithNameW, SetEntriesInAclW, EXPLICIT_ACCESS_W},
            InitializeSecurityDescriptor, SetSecurityDescriptorDacl, PSECURITY_DESCRIPTOR,
            SECURITY_ATTRIBUTES,
        },
        System::Threading::{CreateProcessW, ResumeThread, STARTUPINFOW},
    },
};

#[derive(Parser)]
enum Args {
    Launch {
        #[clap(short, long)]
        game_path: PathBuf,
    },
    Inject,
}

#[derive(Debug)]
struct ProcessInfo {
    pid: u32,
    process_handle: HANDLE,
    thread_handle: HANDLE,
}

impl Drop for ProcessInfo {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.thread_handle);
            CloseHandle(self.process_handle);
        }
    }
}

// ported from Dalamud launch code
unsafe fn spawn_game_process(game_path: PathBuf) -> ProcessInfo {
    let mut explicit_access = std::mem::zeroed::<EXPLICIT_ACCESS_W>();

    let username = std::env::var("USERNAME").unwrap();
    let pcwstr = HSTRING::from(username);

    BuildExplicitAccessWithNameW(
        &mut explicit_access,
        &pcwstr,
        // STANDARD_RIGHTS_ALL | SPECIFIC_RIGHTS_ALL & ~PROCESS_VM_WRITE
        0x001F0000 | 0x0000FFFF & !0x20,
        GRANT_ACCESS,
        ACE_FLAGS(0),
    );

    let mut newacl = std::ptr::null_mut();

    let result = SetEntriesInAclW(Some(&[explicit_access]), None, addr_of_mut!(newacl));
    if result.is_err() {
        panic!("SetEntriesInAclA failed with error code {}", result.0);
    }

    let mut sec_desc = std::mem::zeroed::<SECURITY_DESCRIPTOR>();
    let psec_desc = PSECURITY_DESCRIPTOR(&mut sec_desc as *mut _ as *mut _);
    if !InitializeSecurityDescriptor(psec_desc, 1).as_bool() {
        panic!("InitializeSecurityDescriptor failed");
    }

    if !SetSecurityDescriptorDacl(psec_desc, true, Some(newacl), false).as_bool() {
        panic!("SetSecurityDescriptorDacl failed");
    }

    let mut process_information =
        std::mem::zeroed::<windows::Win32::System::Threading::PROCESS_INFORMATION>();
    let process_attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: psec_desc.0,
        bInheritHandle: BOOL(0),
    };
    let mut startup_info = std::mem::zeroed::<STARTUPINFOW>();
    startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let cmd_line = format!(
        "\"{}\" DEV.TestSID=0 language=1 DEV.MaxEntitledExpansionID=4 DEV.GameQuitMessageBox=0\0",
        game_path.to_str().unwrap()
    );

    let game_dir = game_path.parent().unwrap();

    let res = CreateProcessW(
        None,
        PWSTR(cmd_line.encode_utf16().collect::<Vec<u16>>().as_mut_ptr()),
        Some(&process_attributes),
        None,
        BOOL(0),
        CREATE_SUSPENDED,
        None,
        &HSTRING::from(game_dir.to_str().unwrap()),
        &startup_info,
        &mut process_information,
    );
    let last_error = GetLastError();
    if res == BOOL(0) {
        panic!("CreateProcessW failed with error code {}", last_error);
    }

    // strip SeDebugPrivilege/ACL from the process
    let mut token_handle = std::mem::zeroed::<HANDLE>();

    if !OpenProcessToken(
        process_information.hProcess,
        TOKEN_QUERY | TOKEN_ADJUST_PRIVILEGES,
        &mut token_handle,
    )
    .as_bool()
    {
        panic!("OpenProcessToken failed");
    }

    let mut luid_debug_privilege = std::mem::zeroed::<LUID>();
    if !LookupPrivilegeValueW(None, SE_DEBUG_NAME, &mut luid_debug_privilege).as_bool() {
        panic!("LookupPrivilegeValueW failed");
    }

    let mut required_privileges = PRIVILEGE_SET {
        PrivilegeCount: 1,
        Control: 1,
        Privilege: [LUID_AND_ATTRIBUTES {
            Luid: luid_debug_privilege,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };

    let mut b_result: i32 = 0;
    if !PrivilegeCheck(token_handle, &mut required_privileges, &mut b_result).as_bool() {
        panic!("PrivilegeCheck failed");
    }

    // remove SeDebugPrivilege
    if b_result != 0 {
        println!("removing SeDebugPrivilege");
        let mut token_privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid_debug_privilege,
                Attributes: SE_PRIVILEGE_REMOVED,
            }],
        };

        if !AdjustTokenPrivileges(
            token_handle,
            false,
            Some(&mut token_privileges),
            0,
            None,
            None,
        )
        .as_bool()
        {
            panic!("AdjustTokenPrivileges failed");
        }
    }

    CloseHandle(token_handle);

    ProcessInfo {
        pid: process_information.dwProcessId,
        process_handle: process_information.hProcess,
        thread_handle: process_information.hThread,
    }
}

unsafe fn copy_acl_from_self_to_target(target_process: HANDLE) {
    println!("copying current acl to target process...");

    let mut acl = std::ptr::null_mut() as *mut ACL;

    if !GetSecurityInfo(
        GetCurrentProcess(),
        SE_KERNEL_OBJECT,
        DACL_SECURITY_INFORMATION.0,
        None,
        None,
        Some(addr_of_mut!(acl)),
        None,
        None,
    )
    .is_ok()
    {
        panic!("GetSecurityInfo failed");
    }

    if !SetSecurityInfo(
        target_process,
        SE_KERNEL_OBJECT,
        DACL_SECURITY_INFORMATION.0,
        None,
        None,
        Some(acl),
        None,
    )
    .is_ok()
    {
        panic!("SetSecurityInfo failed");
    }
}

fn await_game_process() -> u32 {
    let pid;

    'wait: loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        let system = sysinfo::System::new_all();
        let processes = system.processes();

        for (_pid, process) in processes {
            if process.name() == "ffxiv_dx11.exe" {
                pid = _pid.as_u32();
                break 'wait;
            }
        }
    }

    pid
}

fn main() {
    let args = Args::parse();

    let process_info;

    match args {
        Args::Launch { game_path } => {
            process_info = unsafe { spawn_game_process(game_path) };
        }
        Args::Inject => {
            process_info = ProcessInfo {
                pid: await_game_process(),
                process_handle: HANDLE(0),
                thread_handle: HANDLE(0),
            };
        }
    }

    println!(
        "pid: {} - tid: {}",
        process_info.pid, process_info.thread_handle.0
    );

    let target;
    if process_info.process_handle.0 != 0 {
        target = unsafe {
            OwnedProcess::from_raw_handle(std::mem::transmute(process_info.process_handle))
        };
    } else {
        target = OwnedProcess::from_pid(process_info.pid).unwrap();
    }

    let syringe = Syringe::for_process(target);

    let current_exe = std::env::current_exe().unwrap();
    let llrt_path = current_exe.join("../grebuloff_llrt.dll");
    let grebuloff_path = current_exe.join("../");

    unsafe {
        if process_info.thread_handle.0 != 0 {
            ResumeThread(process_info.thread_handle);

            if process_info.process_handle.0 != 0 {
                // the idea here is to change the process acl once the window is created,
                // because at that point, the game has already checked its acls.
                // we should actually query to see if the window is open here,
                // but this should suffice for now
                std::thread::sleep(std::time::Duration::from_millis(1000));
                copy_acl_from_self_to_target(process_info.process_handle);
            }
        }
    }

    println!("injecting...");
    let injected_payload = syringe.inject(llrt_path).unwrap();

    println!("calling entrypoint...");
    let remote_load =
        unsafe { syringe.get_payload_procedure::<fn(Vec<u8>)>(injected_payload, "init_native") }
            .unwrap()
            .unwrap();
    let str_as_vec = grebuloff_path.to_str().unwrap().as_bytes().to_vec();
    remote_load.call(&str_as_vec).unwrap();

    println!("done!");
}
