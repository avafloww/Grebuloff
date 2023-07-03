use anyhow::{bail, Result};
use log::info;
use serde::Deserialize;
use serde_json::{Number, Value};
use std::{
    cell::OnceCell,
    collections::HashMap,
    mem, ptr,
    sync::{mpsc, Arc, Mutex},
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    core::*,
    Win32::{
        Foundation::{E_POINTER, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM},
        System::{Com::*, LibraryLoader, Threading, WinRT::EventRegistrationToken},
        UI::WindowsAndMessaging::{
            self, GetForegroundWindow, MSG, WNDCLASSW, WS_EX_LAYERED, WS_OVERLAPPEDWINDOW,
        },
    },
};

static mut WEBVIEW_INSTANCE: OnceCell<WebView> = OnceCell::new();

pub fn init_ui_host() -> Result<()> {
    info!("initializing ui host");

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)?;
    }

    let webview = WebView::create(None, true)?;
    if !unsafe { WEBVIEW_INSTANCE.set(webview.clone()) }.is_ok() {
        bail!("failed to set webview instance");
    }

    webview.init(r#"console.log(`hello world!`);"#).unwrap();

    // Off we go....
    webview.run()
}

struct Window(HWND);

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            WindowsAndMessaging::DestroyWindow(self.0);
        }
    }
}

#[derive(Clone)]
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
                    WS_EX_LAYERED,
                    w!("GrebuloffUIHost"),
                    w!("GrebuloffUIHost"),
                    WS_OVERLAPPEDWINDOW,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    WindowsAndMessaging::CW_USEDEFAULT,
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

type WebViewSender = mpsc::Sender<Box<dyn FnOnce(WebView) + Send>>;
type WebViewReceiver = mpsc::Receiver<Box<dyn FnOnce(WebView) + Send>>;
type BindingCallback = Box<dyn FnMut(Vec<Value>) -> Result<Value>>;
type BindingsMap = HashMap<String, BindingCallback>;

#[derive(Clone)]
pub struct WebView {
    controller: Arc<WebViewController>,
    webview: Arc<ICoreWebView2>,
    tx: WebViewSender,
    rx: Arc<WebViewReceiver>,
    thread_id: u32,
    bindings: Arc<Mutex<BindingsMap>>,
    frame: Option<FrameWindow>,
    parent: Arc<HWND>,
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

        let size = get_window_size(unsafe { GetForegroundWindow() });
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

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(rx);
        let thread_id = unsafe { Threading::GetCurrentThreadId() };

        let webview = WebView {
            controller: Arc::new(WebViewController(controller)),
            webview: Arc::new(webview),
            tx,
            rx,
            thread_id,
            bindings: Arc::new(Mutex::new(HashMap::new())),
            frame,
            parent: Arc::new(parent),
        };

        // Inject the invoke handler.
        webview
            .init(r#"window.external = { invoke: s => window.chrome.webview.postMessage(s) };"#)?;

        let bindings = webview.bindings.clone();
        let bound = webview.clone();
        unsafe {
            let mut _token = EventRegistrationToken::default();
            webview.webview.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_webview, args| {
                    if let Some(args) = args {
                        let mut message = PWSTR(ptr::null_mut());
                        if args.WebMessageAsJson(&mut message).is_ok() {
                            let message = CoTaskMemPWSTR::from(message);
                            if let Ok(value) =
                                serde_json::from_str::<InvokeMessage>(&message.to_string())
                            {
                                if let Ok(mut bindings) = bindings.try_lock() {
                                    if let Some(f) = bindings.get_mut(&value.method) {
                                        match (*f)(value.params) {
                                            Ok(result) => bound.resolve(value.id, 0, result),
                                            Err(err) => bound.resolve(
                                                value.id,
                                                1,
                                                Value::String(format!("{err:#?}")),
                                            ),
                                        }
                                        .unwrap();
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                })),
                &mut _token,
            )?;
        }

        Ok(webview)
    }

    pub fn run(self) -> Result<()> {
        let webview = self.webview.as_ref();
        let (tx, rx) = mpsc::channel();

        let handler = NavigationCompletedEventHandler::create(Box::new(move |_sender, _args| {
            tx.send(()).expect("send over mpsc channel");
            Ok(())
        }));
        let mut token = EventRegistrationToken::default();
        unsafe {
            webview.add_NavigationCompleted(&handler, &mut token)?;
            let url = CoTaskMemPWSTR::from("http://localhost:3000/");
            webview.Navigate(*url.as_ref().as_pcwstr())?;
            let result = webview2_com::wait_with_pump(rx);
            webview.remove_NavigationCompleted(token)?;
            result.unwrap();
        }

        if let Some(frame) = self.frame.as_ref() {
            let hwnd = *frame.window;
            unsafe {
                WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_SHOW);
            }
        }

        let mut msg = MSG::default();
        let h_wnd = HWND::default();

        loop {
            while let Ok(f) = self.rx.try_recv() {
                (f)(self.clone());
            }

            unsafe {
                let result = WindowsAndMessaging::GetMessageW(&mut msg, h_wnd, 0, 0).0;

                match result {
                    -1 => break Err(windows::core::Error::from_win32().into()),
                    0 => break Ok(()),
                    _ => match msg.message {
                        WindowsAndMessaging::WM_APP => (),
                        _ => {
                            WindowsAndMessaging::TranslateMessage(&msg);
                            WindowsAndMessaging::DispatchMessageW(&msg);
                        }
                    },
                }
            }
        }
    }

    pub fn terminate(self) -> Result<()> {
        self.dispatch(|_webview| unsafe {
            WindowsAndMessaging::PostQuitMessage(0);
        })?;

        Ok(())
    }

    pub fn set_size(&self, width: i32, height: i32) -> Result<&Self> {
        if let Some(frame) = self.frame.as_ref() {
            *frame.size.lock().expect("lock size") = SIZE {
                cx: width,
                cy: height,
            };
            unsafe {
                self.controller.0.SetBounds(RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                })?;

                WindowsAndMessaging::SetWindowPos(
                    *frame.window,
                    None,
                    0,
                    0,
                    width,
                    height,
                    WindowsAndMessaging::SWP_NOACTIVATE
                        | WindowsAndMessaging::SWP_NOZORDER
                        | WindowsAndMessaging::SWP_NOMOVE,
                );
            }
        }
        Ok(self)
    }

    pub fn get_window(&self) -> HWND {
        *self.parent
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

    pub fn dispatch<F>(&self, f: F) -> Result<&Self>
    where
        F: FnOnce(WebView) + Send + 'static,
    {
        self.tx.send(Box::new(f)).expect("send the fn");

        unsafe {
            WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WindowsAndMessaging::WM_APP,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
        Ok(self)
    }

    pub fn bind<F>(&self, name: &str, f: F) -> Result<&Self>
    where
        F: FnMut(Vec<Value>) -> Result<Value> + 'static,
    {
        self.bindings
            .lock()
            .unwrap()
            .insert(String::from(name), Box::new(f));

        let js = String::from(
            r#"
            (function() {
                var name = '"#,
        ) + name
            + r#"';
                var RPC = window._rpc = (window._rpc || {nextSeq: 1});
                window[name] = function() {
                    var seq = RPC.nextSeq++;
                    var promise = new Promise(function(resolve, reject) {
                        RPC[seq] = {
                            resolve: resolve,
                            reject: reject,
                        };
                    });
                    window.external.invoke({
                        id: seq,
                        method: name,
                        params: Array.prototype.slice.call(arguments),
                    });
                    return promise;
                }
            })()"#;

        self.init(&js)
    }

    pub fn resolve(&self, id: u64, status: i32, result: Value) -> Result<&Self> {
        let result = result.to_string();

        self.dispatch(move |webview| {
            let method = match status {
                0 => "resolve",
                _ => "reject",
            };
            let js = format!(
                r#"
                window._rpc[{id}].{method}({result});
                window._rpc[{id}] = undefined;"#
            );

            webview.eval(&js).expect("eval return script");
        })
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
            unsafe {
                WindowsAndMessaging::DestroyWindow(hwnd);
            }
            LRESULT::default()
        }

        WindowsAndMessaging::WM_DESTROY => {
            // webview.terminate().expect("window is gone");
            LRESULT::default()
        }

        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, w_param, l_param) },
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
