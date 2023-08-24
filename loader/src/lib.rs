use plthook::{ObjectFile, Replacement};
use std::{
    ffi::{c_void, CString},
    mem::ManuallyDrop,
    os::windows::prelude::OsStringExt,
    path::PathBuf,
};
use windows::{
    core::{ComInterface, PCSTR},
    Win32::{
        Foundation::HANDLE,
        Graphics::Dxgi::IDXGIFactory2,
        System::LibraryLoader::{GetModuleFileNameW, LoadLibraryA},
    },
};
use windows::{
    core::{HRESULT, HSTRING},
    Win32::{
        Foundation::HWND,
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryExA, LOAD_WITH_ALTERED_SEARCH_PATH},
            SystemServices::DLL_PROCESS_ATTACH,
        },
        UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK},
    },
};

const ROOT_ENV_VAR: &'static str = "GREBULOFF_ROOT";
const ROOT_FILE: &'static str = "grebuloff_root.txt";

const ERROR_NO_ROOT: &'static str = r#"Could not find the Grebuloff root directory!

We checked the following locations:
1. "GREBULOFF_ROOT" environment variable passed to the game executable
2. "grebuloff_root.txt" in the same directory as the game executable
3. The default installation directory: %AppData%\Grebuloff

None of the paths searched contained a valid Grebuloff installation, so loading cannot continue.
If you are trying to uninstall Grebuloff, delete "winhttp.dll" from the game directory.

The game will now exit."#;

static mut IAT_HOOK: Option<ManuallyDrop<Replacement>> = None;

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(
    _hinstDLL: HANDLE,
    fdwReason: u32,
    _lpvReserved: *const std::ffi::c_void,
) -> bool {
    if fdwReason == DLL_PROCESS_ATTACH {
        // make sure we're in the right damn process...
        if !get_exe_path()
            .file_name()
            .unwrap()
            .eq_ignore_ascii_case("ffxiv_dx11.exe")
        {
            return false;
        }

        let wakeup_cnt = std::env::var("FFIXV_WAKEUP_CNT");
        if !wakeup_cnt.is_ok() {
            // FFXIV sets this env var for the fork where it executes for real.
            // If the env var isn't set, the process we just loaded into is about to
            // get restarted, so we should just exit.
            return true;
        }

        // we redirect CreateDXGIFactory here and return, so that we can load Grebuloff
        // it's expressly forbidden to load libraries in DllMain, and has a tendency to deadlock
        redirect_dxgi();
    }

    true
}

unsafe fn load_grebuloff() {
    let root = get_grebuloff_root();
    let load_result = LoadLibraryExA(root.dll_path, None, LOAD_WITH_ALTERED_SEARCH_PATH);

    match load_result {
        Ok(dll) => {
            // get the address of init_loader
            let init_loader: Option<fn(&CString) -> ()> =
                GetProcAddress(dll, PCSTR::from_raw(b"init_loader\0".as_ptr()))
                    .map(|func| std::mem::transmute(func));

            match init_loader {
                Some(init_loader) => {
                    // call init_loader
                    let runtime_dir = CString::new(root.runtime_path).unwrap();
                    init_loader(&runtime_dir);
                }
                None => {
                    display_error(&format!(
                        r#"Failed to find init_loader in Grebuloff at {}!

The game will now exit."#,
                        root.dll_path.to_string().unwrap(),
                    ));
                    std::process::exit(3);
                }
            }
        }
        Err(e) => {
            display_error(&format!(
                r#"Failed to load Grebuloff at {}!

The error was: {:?}

The game will now exit."#,
                root.dll_path.to_string().unwrap(),
                e
            ));
            std::process::exit(2);
        }
    }
}

unsafe extern "system" fn create_dxgi_factory_wrapper(
    _riid: *const (),
    pp_factory: *mut *mut c_void,
) -> HRESULT {
    // remove the IAT hook now that we've been called
    if let Some(hook) = IAT_HOOK.take() {
        ManuallyDrop::into_inner(hook);
    } else {
        display_error("...huh? IAT_HOOK was None...");
        std::process::exit(5);
    }

    // load Grebuloff
    load_grebuloff();

    // call CreateDXGIFactory1 from dxgi.dll
    // we use CreateDXGIFactory1 instead of CreateDXGIFactory, passing in IDXGIFactory2 as the riid,
    // to create a DXGI 1.2 factory, as opposed to the DXGI 1.0 factory that the game creates
    // this shouldn't break anything, but it does allow us to use surface sharing from Chromium
    // (once we implement that), for high-performance UI rendering
    let dxgi_dll = LoadLibraryA(PCSTR::from_raw(b"dxgi.dll\0".as_ptr())).unwrap();
    let original_func: Option<fn(*const _, *mut *mut c_void) -> HRESULT> =
        GetProcAddress(dxgi_dll, PCSTR::from_raw(b"CreateDXGIFactory1\0".as_ptr()))
            .map(|func| std::mem::transmute(func));

    // CreateDXGIFactory1()
    match original_func {
        Some(original_func) => original_func(&IDXGIFactory2::IID, pp_factory),
        None => {
            display_error("...huh? failed to find CreateDXGIFactory1 in dxgi.dll...");
            std::process::exit(4);
        }
    }
}

fn display_error(msg: &str) {
    let msg = HSTRING::from(msg);
    let title = HSTRING::from("Grebuloff Loader");
    unsafe {
        MessageBoxW(HWND::default(), &msg, &title, MB_OK | MB_ICONERROR);
    }
}

struct GrebuloffRoot {
    runtime_path: String,
    dll_path: PCSTR,
}

impl TryFrom<String> for GrebuloffRoot {
    type Error = ();

    fn try_from(runtime_path: String) -> Result<Self, Self::Error> {
        let mut dll_path = std::path::PathBuf::from(&runtime_path);
        dll_path.push("grebuloff.dll");

        if !dll_path.exists() {
            return Err(());
        }

        let dll_path = dll_path.to_str().unwrap().to_owned();

        Ok(GrebuloffRoot {
            runtime_path,
            dll_path: PCSTR::from_raw(dll_path.as_ptr()),
        })
    }
}

fn get_exe_path() -> PathBuf {
    unsafe {
        let mut exe_path = [0u16; 1024];
        let exe_path_len = GetModuleFileNameW(None, &mut exe_path);

        std::path::PathBuf::from(std::ffi::OsString::from_wide(
            &exe_path[..exe_path_len as usize],
        ))
    }
}

fn get_grebuloff_root() -> GrebuloffRoot {
    // try in this order:
    // 1. `GREBULOFF_ROOT` env var, if set
    // 2. `grebuloff_root.txt` in the same directory as the game's EXE
    // 3. default to %AppData%\Grebuloff
    // if none of these exist, we can't continue - display an error message and exit
    std::env::var(ROOT_ENV_VAR)
        .or_else(|_| {
            // usually we'll be in the game directory, but we might not be
            // ensure we search for grebuloff_root.txt in the game directory
            std::fs::read_to_string(
                get_exe_path()
                    .parent()
                    .map(|p| p.join(ROOT_FILE))
                    .unwrap_or(ROOT_FILE.into()),
            )
            .map(|s| s.trim().to_owned())
        })
        .or_else(|_| {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let mut path = std::path::PathBuf::from(appdata);
                path.push("Grebuloff");
                if path.exists() {
                    return Ok(path.to_str().map(|s| s.to_owned()).unwrap());
                }
            }

            Err(())
        })
        .map(GrebuloffRoot::try_from)
        .unwrap_or_else(|_| {
            display_error(ERROR_NO_ROOT);
            std::process::exit(1);
        })
        .unwrap()
}

unsafe fn redirect_dxgi() {
    let source = CString::new("CreateDXGIFactory").unwrap();
    let obj = ObjectFile::open_main_program().unwrap();

    for symbol in obj.symbols() {
        if symbol.name == source {
            // replace the address of CreateDXGIFactory with our own init function
            let _ = IAT_HOOK.insert(ManuallyDrop::new(
                obj.replace(
                    source.to_str().unwrap(),
                    create_dxgi_factory_wrapper as *const _,
                )
                .unwrap(),
            ));

            break;
        }
    }
}
