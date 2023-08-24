use std::sync::Arc;

use crate::{
    refcounted::{wrap_ptr, BaseRefCountedExt, WrapperFor},
    ToCef,
};

pub trait Client {}

struct ClientWrapper<T: Client> {
    _base: _cef_client_t,
    internal: Arc<T>,
}
unsafe impl<T: Client> WrapperFor<_cef_client_t> for ClientWrapper<T> {}
impl<T: Client> ClientWrapper<T> {
    fn from_ptr<'a>(
        ptr: *mut _cef_client_t,
    ) -> &'a mut BaseRefCountedExt<_cef_client_t, ClientWrapper<T>> {
        unsafe { &mut *(ptr as *mut _) }
    }
}

impl<T: Client> ToCef<_cef_client_t> for Arc<T> {
    fn to_cef(&self) -> *mut _cef_client_t {
        wrap_ptr(|base| ClientWrapper {
            _base: _cef_client_t {
                base,
                get_audio_handler: None,
                get_command_handler: None,
                get_context_menu_handler: None,
                get_dialog_handler: None,
                get_display_handler: None,
                get_download_handler: None,
                get_drag_handler: None,
                get_find_handler: None,
                get_focus_handler: None,
                get_frame_handler: None,
                get_permission_handler: None,
                get_jsdialog_handler: None,
                get_keyboard_handler: None,
                get_life_span_handler: None,
                get_load_handler: None,
                get_print_handler: None,
                get_render_handler: None,
                get_request_handler: None,
                on_process_message_received: None,
            },
            internal: self.clone(),
        })
    }
}
