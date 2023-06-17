use std::{
    collections::HashMap,
    ptr,
    sync::{mpsc, Arc, Mutex},
};

use log::info;
use serde::Deserialize;
use serde_json::Value;
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, CoTaskMemPWSTR,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    ExecuteScriptCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2Controller,
        ICoreWebView2Controller2, COREWEBVIEW2_COLOR,
    },
    NavigationCompletedEventHandler, WebMessageReceivedEventHandler,
};
use windows::{
    core::ComInterface,
    core::PWSTR,
    Win32::{
        Foundation::{HWND, LPARAM, RECT, SIZE, WPARAM},
        System::{
            Com::{CoInitializeEx, COINIT_APARTMENTTHREADED},
            Threading,
            WinRT::EventRegistrationToken,
        },
        UI::WindowsAndMessaging::{self, MSG},
    },
};

fn get_window_size(hwnd: HWND) -> SIZE {
    let mut client_rect = RECT::default();
    unsafe { WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect as *mut RECT) };
    SIZE {
        cx: client_rect.right - client_rect.left,
        cy: client_rect.bottom - client_rect.top,
    }
}

// Stolen from the webview2-rs example
// https://github.com/wravery/webview2-rs/blob/main/crates/webview2-com/examples/sample.rs#L270
struct WebViewController(ICoreWebView2Controller);
type WebViewSender = mpsc::Sender<Box<dyn FnOnce(WebView) + Send>>;
type WebViewReceiver = mpsc::Receiver<Box<dyn FnOnce(WebView) + Send>>;
type BindingCallback = Box<dyn FnMut(Vec<Value>) -> Result<Value, String> + Send>;
type BindingsMap = HashMap<String, BindingCallback>;

#[derive(Clone)]
pub struct WebView {
    controller: Arc<WebViewController>,
    webview: Arc<ICoreWebView2>,
    tx: WebViewSender,
    rx: Arc<WebViewReceiver>,
    thread_id: u32,
    bindings: Arc<Mutex<BindingsMap>>,
    parent: Arc<HWND>,
    url: Arc<Mutex<String>>,
}

#[derive(Debug, Deserialize)]
struct InvokeMessage {
    id: u64,
    method: String,
    params: Vec<Value>,
}

impl WebView {
    pub fn new() -> Self {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).expect("Failed to initialize COM");
        }

        let hwnd = unsafe { WindowsAndMessaging::GetForegroundWindow() };

        let environment = {
            let (tx, rx) = mpsc::channel();

            CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(|environmentcreatedhandler| unsafe {
                    CreateCoreWebView2Environment(&environmentcreatedhandler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, environment| {
                    error_code?;
                    tx.send(environment.expect("Failed to get environment"))
                        .expect("Failed to send environment");
                    Ok(())
                }),
            )
            .expect("Failed to create environment");

            rx.recv().expect("Failed to receive environment")
        };

        let controller = {
            let (tx, rx) = mpsc::channel();

            CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| unsafe {
                    environment
                        .CreateCoreWebView2Controller(hwnd, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, controller| {
                    error_code?;
                    tx.send(controller.expect("Failed to get controller"))
                        .expect("Failed to send controller");
                    Ok(())
                }),
            )
            .expect("Failed to create controller");

            rx.recv().expect("Failed to receive controller")
        };

        let size = get_window_size(hwnd);
        let mut client_rect = RECT::default();
        unsafe {
            WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect as *mut RECT);
            let controller2: ICoreWebView2Controller2 =
                controller.cast().expect("Failed to cast to controller2");
            controller2
                .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                    R: 0,
                    G: 0,
                    B: 0,
                    A: 0,
                })
                .expect("Failed to set background to transparent");

            controller
                .SetBounds(RECT {
                    left: 0,
                    top: 0,
                    right: size.cx,
                    bottom: size.cy,
                })
                .expect("Failed to set bounds");
            controller
                .SetIsVisible(true)
                .expect("Failed to set visibility");
        }

        let webview = unsafe { controller.CoreWebView2().expect("Failed to get webview") };

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
            parent: Arc::new(hwnd),
            // todo: move this url to config - in release builds we want to serve from a local file
            url: Arc::new(Mutex::new(String::from("http://localhost:3000/"))),
        };

        webview
            .init(r#"window.external = { invoke: s => window.chrome.webview.postMessage(s) };"#)
            .expect("Failed to initialize webview");

        let bindings = webview.bindings.clone();
        let bound = webview.clone();
        unsafe {
            let mut _token = EventRegistrationToken::default();
            webview
                .webview
                .add_WebMessageReceived(
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
                )
                .expect("Failed to add WebMessageReceived handler");
        }

        webview
    }

    fn init(&self, js: &str) -> Result<&Self, webview2_com::Error> {
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
        )?;

        Ok(self)
    }

    fn resolve(&self, id: u64, status: i32, result: Value) -> Result<&Self, webview2_com::Error> {
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

    fn dispatch<F>(&self, f: F) -> Result<&Self, webview2_com::Error>
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

    fn eval(&self, js: &str) -> Result<&Self, webview2_com::Error> {
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
        )?;
        Ok(self)
    }

    pub fn run(self) -> Result<(), webview2_com::Error> {
        let webview = self.webview.as_ref();
        let url = self.url.try_lock().expect("Failed to lock url").clone();
        let (tx, rx) = mpsc::channel();

        if !url.is_empty() {
            let handler =
                NavigationCompletedEventHandler::create(Box::new(move |_sender, _args| {
                    tx.send(()).expect("send over mpsc channel");
                    Ok(())
                }));
            let mut token = EventRegistrationToken::default();
            unsafe {
                webview.add_NavigationCompleted(&handler, &mut token)?;
                let url = CoTaskMemPWSTR::from(url.as_str());
                webview.Navigate(*url.as_ref().as_pcwstr())?;
                let result = webview2_com::wait_with_pump(rx);
                webview.remove_NavigationCompleted(token)?;
                result?;
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
}
