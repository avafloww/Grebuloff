use anyhow::{bail, Result};
use log::{debug, info, trace};
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::{OnceCell, RefCell},
    collections::{HashMap, VecDeque},
    mem::ManuallyDrop,
    ptr,
    sync::{mpsc, Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use win_screenshot::prelude::{capture_window_ex, Area, RgbBuf, Using};
use windows::{
    core::*,
    Win32::{
        Foundation::{E_POINTER, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM},
        Graphics::{Direct3D11::D3D11_SUBRESOURCE_DATA, Gdi},
        System::{Com::*, LibraryLoader, Threading, WinRT::EventRegistrationToken},
        UI::WindowsAndMessaging::{
            self, GetForegroundWindow, MSG, PM_REMOVE, WNDCLASSW, WS_DISABLED, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_OVERLAPPEDWINDOW,
        },
    },
};

static mut WEBVIEW_INSTANCE: OnceCell<Arc<WebView>> = OnceCell::new();

pub fn init_ui_host() -> Result<()> {
    info!("initializing ui host");

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)?;
    }

    let webview = Arc::new(WebView::create(None, true).unwrap());
    if !unsafe { WEBVIEW_INSTANCE.set(webview.clone()) }.is_ok() {
        bail!("failed to set webview instance");
    }

    webview.init(r#"console.log(`hello world!`);"#).unwrap();

    // Off we go....
    webview.run().unwrap();

    Ok(())
}

struct Window(HWND);

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            WindowsAndMessaging::DestroyWindow(self.0);
        }
    }
}

pub struct FrameWindow {
    window: Arc<HWND>,
    size: Arc<Mutex<SIZE>>,
}

impl FrameWindow {
    fn new() -> Self {
        let hwnd = {
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                lpszClassName: w!("GrebuloffUIHost"),
                ..Default::default()
            };

            unsafe {
                WindowsAndMessaging::RegisterClassW(&window_class);

                WindowsAndMessaging::CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_NOACTIVATE,
                    w!("GrebuloffUIHost"),
                    w!("GrebuloffUIHost"),
                    WS_DISABLED,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    1920 + 6,  //WindowsAndMessaging::CW_USEDEFAULT,
                    1080 + 40, //WindowsAndMessaging::CW_USEDEFAULT,
                    None,
                    None,
                    LibraryLoader::GetModuleHandleW(None).unwrap_or_default(),
                    None,
                )
            }
        };

        FrameWindow {
            window: Arc::new(hwnd),
            size: Arc::new(Mutex::new(SIZE { cx: 0, cy: 0 })),
        }
    }
}

struct WebViewController(ICoreWebView2Controller);

type BindingCallback = Box<dyn FnMut(Vec<Value>) -> Result<Value>>;
type BindingsMap = HashMap<String, BindingCallback>;

pub struct WebView {
    controller: WebViewController,
    webview: ICoreWebView2,
    thread_id: u32,
    bindings: Mutex<BindingsMap>,
    frame: Option<FrameWindow>,
    parent: HWND,
    pub capture: WebViewCapture,
}

impl Drop for WebViewController {
    fn drop(&mut self) {
        unsafe { self.0.Close() }.unwrap();
    }
}

#[derive(Debug, Deserialize)]
struct InvokeMessage {
    id: u64,
    method: String,
    params: Vec<Value>,
}

impl WebView {
    pub fn create(parent: Option<HWND>, debug: bool) -> Result<WebView> {
        let (parent, frame) = match parent {
            Some(hwnd) => (hwnd, None),
            None => {
                let frame = FrameWindow::new();
                (*frame.window, Some(frame))
            }
        };

        let environment = {
            let (tx, rx) = mpsc::channel();

            CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(|environmentcreatedhandler| unsafe {
                    CreateCoreWebView2Environment(&environmentcreatedhandler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, environment| {
                    error_code?;
                    tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                        .expect("send over mpsc channel");
                    Ok(())
                }),
            )
            .unwrap();

            rx.recv()?
        }?;

        let controller = {
            let (tx, rx) = mpsc::channel();

            CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| unsafe {
                    environment
                        .CreateCoreWebView2Controller(parent, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, controller| {
                    error_code?;
                    tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                        .expect("send over mpsc channel");
                    Ok(())
                }),
            )
            .unwrap();

            rx.recv()?
        }?;

        let size = get_window_size(parent);
        let mut client_rect = RECT::default();
        unsafe {
            WindowsAndMessaging::GetClientRect(parent, std::mem::transmute(&mut client_rect));
            controller
                .cast::<ICoreWebView2Controller2>()
                .unwrap()
                .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                    R: 0,
                    G: 0,
                    B: 0,
                    A: 0,
                })
                .expect("Failed to set background to transparent");
            controller.SetBounds(RECT {
                left: 0,
                top: 0,
                right: size.cx,
                bottom: size.cy,
            })?;
            controller.SetIsVisible(true)?;
        }

        let webview = unsafe { controller.CoreWebView2()? };

        if !debug {
            unsafe {
                let settings = webview.Settings()?;
                settings.SetAreDefaultContextMenusEnabled(false)?;
                settings.SetAreDevToolsEnabled(false)?;
            }
        }

        if let Some(frame) = frame.as_ref() {
            *frame.size.lock().unwrap() = size;
        }

        let thread_id = unsafe { Threading::GetCurrentThreadId() };

        let webview = WebView {
            controller: WebViewController(controller),
            webview,
            thread_id,
            bindings: Mutex::new(HashMap::new()),
            frame,
            parent,
            capture: WebViewCapture::new(parent, 60),
        };

        Ok(webview)
    }

    fn run(self: Arc<Self>) -> Result<()> {
        unsafe {
            let url = CoTaskMemPWSTR::from("http://localhost:3000/");
            self.webview.Navigate(*url.as_ref().as_pcwstr())?;
        }

        if let Some(frame) = self.frame.as_ref() {
            let hwnd = *frame.window;
            unsafe {
                WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_HIDE);
                Gdi::UpdateWindow(hwnd);
            }
        }

        let mut msg = MSG::default();
        let hwnd = self.parent;
        let mut last_summary = Instant::now();

        info!("starting ui host main loop");

        loop {
            // handle Windows messages
            unsafe {
                let result = WindowsAndMessaging::PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE).0;

                match result {
                    -1 => {
                        // break Err(windows::core::Error::from_win32().into()),
                        let error = windows::core::Error::from_win32();
                        if error.code().0 == 0 {
                            // work around what's probably a Windows bug by ceremoniously ignoring this fake error
                        } else {
                            break Err(error.into());
                        }
                    }
                    // 0 => break Ok(()),
                    _ => match msg.message {
                        WindowsAndMessaging::WM_APP => (),
                        _ => {
                            WindowsAndMessaging::TranslateMessage(&msg);
                            WindowsAndMessaging::DispatchMessageW(&msg);
                        }
                    },
                }
            }

            // capture if necessary
            self.capture.capture_or_sleep()?;

            if last_summary.elapsed() > Duration::from_secs(10) {
                last_summary = Instant::now();
                let (min, max, avg) = self.capture.frame_time_stats();
                debug!(
                    "UI capture: frame time summary: min {}ms, max {}ms, avg {}ms",
                    min, max, avg
                );
            }
        }
    }

    pub fn init(&self, js: &str) -> Result<&Self> {
        let webview = self.webview.clone();
        let js = String::from(js);
        AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                let js = CoTaskMemPWSTR::from(js.as_str());
                webview
                    .AddScriptToExecuteOnDocumentCreated(*js.as_ref().as_pcwstr(), &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(|error_code, _id| error_code),
        )
        .unwrap();
        Ok(self)
    }

    pub fn eval(&self, js: &str) -> Result<&Self> {
        let webview = self.webview.clone();
        let js = String::from(js);
        ExecuteScriptCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                let js = CoTaskMemPWSTR::from(js.as_str());
                webview
                    .ExecuteScript(*js.as_ref().as_pcwstr(), &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(|error_code, _result| error_code),
        )
        .unwrap();
        Ok(self)
    }

    pub fn instance() -> Option<Arc<WebView>> {
        unsafe { WEBVIEW_INSTANCE.get().cloned() }
    }
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    let webview = match unsafe { WEBVIEW_INSTANCE.get() } {
        Some(webview) => webview,
        None => return unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, w_param, l_param) },
    };

    let frame = webview
        .frame
        .as_ref()
        .expect("should only be called for owned windows");

    match msg {
        WindowsAndMessaging::WM_SIZE => {
            let size = get_window_size(hwnd);
            unsafe {
                webview
                    .controller
                    .0
                    .SetBounds(RECT {
                        left: 0,
                        top: 0,
                        right: size.cx,
                        bottom: size.cy,
                    })
                    .unwrap();
            }
            *frame.size.lock().expect("lock size") = size;
            LRESULT::default()
        }

        WindowsAndMessaging::WM_CLOSE => {
            info!("WM_CLOSE");
            unsafe {
                WindowsAndMessaging::DestroyWindow(hwnd);
            }
            LRESULT::default()
        }

        WindowsAndMessaging::WM_DESTROY => {
            // webview.terminate().expect("window is gone");
            info!("WM_CLOSE");
            LRESULT::default()
        }

        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, w_param, l_param) },
    }
}

pub struct WebViewCapture {
    hwnd: HWND,
    target_fps: u32,
    frame_times_capacity: usize,
    last_frame: RwLock<Option<WebViewCaptureFrame>>,
    state: RwLock<WebViewCaptureState>,
}

pub struct WebViewCaptureState {
    last_frame_time: Instant,
    last_frame_times: VecDeque<u16>,
}

impl WebViewCapture {
    fn new(hwnd: HWND, target_fps: u32) -> Self {
        let frame_times_capacity = target_fps as usize * 5;
        WebViewCapture {
            hwnd,
            target_fps,
            frame_times_capacity,
            last_frame: RwLock::new(None),
            state: RwLock::new(WebViewCaptureState {
                last_frame_time: Instant::now() - Duration::from_secs(1),
                last_frame_times: VecDeque::with_capacity(frame_times_capacity),
            }),
        }
    }

    fn capture_or_sleep(&self) -> Result<()> {
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.state.read().unwrap().last_frame_time)
            .as_secs_f64();
        let time_between_frames = 1.0 / self.target_fps as f64;

        if elapsed > time_between_frames {
            self.capture()?;
        } else {
            // sleep for the remaining time until next capture
            let duration = Duration::from_secs_f64(time_between_frames - elapsed);
            trace!("capture_or_sleep: sleeping for {:?}", duration);
            std::thread::sleep(duration);
        }

        Ok(())
    }

    fn capture(&self) -> Result<()> {
        let mut state = self.state.write().unwrap();
        let start = Instant::now();
        state.last_frame_time = start;

        // let hwnd = win_screenshot::utils::find_window(r"C:\").unwrap();

        let frame = capture_window_ex(
            self.hwnd.0,
            Using::PrintWindow,
            Area::ClientOnly,
            None,
            None,
        )?;

        if true {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let name = format!("test-{}.png", now);

            debug!("saving {}", name);

            // unix timestamp

            let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels.clone())
                .unwrap();

            img.save(name).unwrap();
        }

        // save the frame to the last_frame
        *self.last_frame.write().unwrap() = Some(WebViewCaptureFrame::new(&frame));

        // record the time it took
        let elapsed = start.elapsed().as_millis() as u16;
        state
            .last_frame_times
            .truncate(self.frame_times_capacity - 1);
        state.last_frame_times.push_front(elapsed);

        Ok(())
    }

    /// Returns the min, max and average frame time in milliseconds.
    pub fn frame_time_stats(&self) -> (u16, u16, u16) {
        let state = &self.state.read().unwrap();

        let mut min = u16::MAX;
        let mut max = u16::MIN;
        let mut sum = 0 as u64;

        for time in &state.last_frame_times {
            if time < &min {
                min = *time;
            }
            if time > &max {
                max = *time;
            }
            sum += *time as u64;
        }

        let avg = sum / state.last_frame_times.len() as u64;

        (min, max, avg as u16)
    }

    pub fn get_last_frame(&self) -> Option<WebViewCaptureFrame> {
        let state = self.last_frame.read().unwrap();
        state.clone()
    }
}

#[derive(Clone)]
pub struct WebViewCaptureFrame {
    pub pixels: Box<[u8]>,
    pub width: u32,
    pub height: u32,
}

impl WebViewCaptureFrame {
    fn new(buf: &RgbBuf) -> Self {
        Self {
            pixels: buf.pixels.clone().into(),
            width: buf.width,
            height: buf.height,
        }
    }

    pub fn into_subresource_data(self) -> D3D11_SUBRESOURCE_DATA {
        D3D11_SUBRESOURCE_DATA {
            pSysMem: self.pixels.as_ptr() as *const _,
            SysMemPitch: self.width,
            SysMemSlicePitch: self.pixels.len() as u32, // only applies to 3D textures
        }
    }
}

fn get_window_size(hwnd: HWND) -> SIZE {
    let mut client_rect = RECT::default();
    unsafe { WindowsAndMessaging::GetClientRect(hwnd, std::mem::transmute(&mut client_rect)) };
    SIZE {
        cx: client_rect.right - client_rect.left,
        cy: client_rect.bottom - client_rect.top,
    }
}
