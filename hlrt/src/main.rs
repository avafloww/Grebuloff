use cef_sys::cef_currently_on;
use client::Client;
use std::{ffi::*, mem::size_of, ptr::null_mut, sync::Arc};
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Gdi::ValidateRect,
    Win32::System::LibraryLoader::GetModuleHandleA, Win32::UI::WindowsAndMessaging::*,
};

pub type HINSTANCE = HMODULE;

pub mod client;
pub mod string;

pub trait ToCef<T> {
    fn to_cef(&self) -> *mut T;
}

pub fn require_ui_thread() {
    unsafe {
        if cef_currently_on(cef_thread_id_t_TID_UI) == 0 {
            log::warn!("Not on UI thread!");
        }
    }
}

fn get_instance() -> HINSTANCE {
    unsafe {
        let instance = GetModuleHandleA(None).unwrap();
        debug_assert!(instance.0 != 0);

        instance.into()
    }
}

fn create_window() -> HWND {
    unsafe {
        let instance = get_instance();
        let window_class = s!("window");

        let wc = WNDCLASSA {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hInstance: instance,
            lpszClassName: window_class,

            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        };

        let atom = RegisterClassA(&wc);
        debug_assert!(atom != 0);

        let hwnd = CreateWindowExA(
            WINDOW_EX_STYLE::default(),
            window_class,
            s!("HLRT"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            instance,
            None,
        );

        hwnd
    }
}

unsafe extern "C" fn dtr(_: *mut char16) {}

pub fn cef_string_empty() -> cef_string_t {
    // let mut empty_str = cef_string_t {
    //     str_: null_mut(),
    //     length: 0,
    //     dtor: Some(dtr),
    // };

    // let emp = "";
    // unsafe {
    //     cef_string_utf8_to_utf16(emp.as_ptr() as *mut c_char, 0, &mut empty_str);
    // }

    // empty_str
    unsafe { std::mem::zeroed() }
}

pub fn cef_string(value: &str) -> cef_string_t {
    let mut str_cef = cef_string_t {
        str_: null_mut(),
        length: 0,
        dtor: Some(dtr),
    };
    unsafe {
        cef_string_utf8_to_utf16(value.as_ptr() as *mut c_char, value.len(), &mut str_cef);
    }
    str_cef
}

pub fn create_browser(
    // canvas_hwnd: HWND,
    client: Arc<OurClient>,
    url: &str,
    // w: i32,
    // h: i32,
    bg: cef_color_t,
) -> *mut cef_browser_t {
    let window_info = cef_window_info(); //canvas_hwnd, w, h);
                                         // Browser settings.
    let browser_settings = _cef_browser_settings_t {
        size: size_of::<_cef_browser_settings_t>(),
        windowless_frame_rate: 0,
        standard_font_family: cef_string_empty(),
        fixed_font_family: cef_string_empty(),
        serif_font_family: cef_string_empty(),
        sans_serif_font_family: cef_string_empty(),
        cursive_font_family: cef_string_empty(),
        fantasy_font_family: cef_string_empty(),
        default_font_size: 0,
        default_fixed_font_size: 0,
        minimum_font_size: 0,
        minimum_logical_font_size: 0,
        default_encoding: cef_string_empty(),
        remote_fonts: cef_state_t::STATE_DEFAULT,
        javascript: cef_state_t::STATE_DEFAULT,
        javascript_close_windows: cef_state_t::STATE_DEFAULT,
        javascript_access_clipboard: cef_state_t::STATE_DEFAULT,
        javascript_dom_paste: cef_state_t::STATE_DEFAULT,
        image_loading: cef_state_t::STATE_DEFAULT,
        image_shrink_standalone_to_fit: cef_state_t::STATE_DEFAULT,
        text_area_resize: cef_state_t::STATE_DEFAULT,
        tab_to_links: cef_state_t::STATE_DEFAULT,
        local_storage: cef_state_t::STATE_DEFAULT,
        databases: cef_state_t::STATE_DEFAULT,
        webgl: cef_state_t::STATE_DEFAULT,
        background_color: bg,
        accept_language_list: cef_string_empty(),
        chrome_status_bubble: cef_state_t::STATE_DISABLED,
    };

    let url_cef = cef_string(url);

    // Create browser.
    let browser: *mut cef_browser_t = unsafe {
        cef_browser_host_create_browser_sync(
            &window_info,
            client.to_cef(), //null_mut(), // jclient,
            &url_cef,
            &browser_settings,
            null_mut(),
            null_mut(),
        )
    };
    assert_eq!(unsafe { (*browser).base.size }, size_of::<_cef_browser_t>());
    browser
}

fn cef_window_info(/*hwnd: HWND, w: i32, h: i32*/) -> _cef_window_info_t {
    _cef_window_info_t {
        bounds: _cef_rect_t {
            x: CW_USEDEFAULT,
            y: CW_USEDEFAULT,
            width: CW_USEDEFAULT,
            height: CW_USEDEFAULT,
        },
        parent_window: HWND::default(), //hwnd,
        windowless_rendering_enabled: 0,
        window: HWND::default(),
        ex_style: 0,
        window_name: cef_string("cef"),
        style: WS_OVERLAPPEDWINDOW.0 | WS_CLIPCHILDREN.0 | WS_CLIPSIBLINGS.0 | WS_VISIBLE.0,
        menu: HMENU::default(),
        shared_texture_enabled: 0,
        external_begin_frame_enabled: 0,
    }
}

fn cef_settings() -> _cef_settings_t {
    _cef_settings_t {
        size: std::mem::size_of::<_cef_settings_t>(),
        no_sandbox: 1,
        browser_subprocess_path: cef_string_empty(),
        framework_dir_path: cef_string_empty(),
        multi_threaded_message_loop: 0,
        external_message_pump: 0,
        windowless_rendering_enabled: 0,
        command_line_args_disabled: 0,
        cache_path: cef_string_empty(),
        root_cache_path: cef_string_empty(),
        user_data_path: cef_string_empty(),
        persist_session_cookies: 0,
        persist_user_preferences: 0,
        user_agent: cef_string_empty(),
        locale: cef_string_empty(),
        log_file: cef_string_empty(),
        log_severity: cef_log_severity_t::LOGSEVERITY_VERBOSE,
        javascript_flags: cef_string_empty(),
        resources_dir_path: cef_string_empty(),
        locales_dir_path: cef_string_empty(),
        pack_loading_disabled: 0,
        remote_debugging_port: 9696,
        uncaught_exception_stack_size: 0,
        background_color: 0,
        accept_language_list: cef_string_empty(),
        main_bundle_path: cef_string_empty(),
        chrome_runtime: 0,
        user_agent_product: cef_string_empty(),
        cookieable_schemes_list: cef_string_empty(),
        cookieable_schemes_exclude_defaults: 0,
    }
}

pub struct OurClient;
impl Client for OurClient {}

fn main() -> Result<()> {
    unsafe {
        let instance = get_instance();

        // exec subprocess, maybe
        {
            let exit_code =
                cef_execute_process(&cef_main_args_t { instance }, null_mut(), null_mut());

            if exit_code >= 0 {
                std::process::exit(exit_code);
            }
        }

        // let window = create_window();

        // let mut rect = RECT::default();
        // GetClientRect(window, &mut rect);

        // println!("rect: {:?}", rect);

        let cef_args = _cef_main_args_t { instance };

        let cef_settings = cef_settings();
        println!("cef init");
        cef_initialize(&cef_args, &cef_settings, null_mut(), null_mut());

        println!("cef create");
        let client = Arc::new(OurClient);
        create_browser(
            // window,
            client.clone(),
            "https://google.com",
            // rect.right,
            // rect.bottom,
            0,
        );

        println!("msg loop");

        cef_run_message_loop();
        // // process window messages
        // let mut message = MSG::default();
        // while GetMessageA(&mut message, None, 0, 0).into() {
        //     DispatchMessageA(&message);
        // }

        Ok(())
    }
}

extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_PAINT => {
                println!("WM_PAINT");
                ValidateRect(window, None);
                LRESULT(0)
            }
            WM_DESTROY => {
                println!("WM_DESTROY");
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}
