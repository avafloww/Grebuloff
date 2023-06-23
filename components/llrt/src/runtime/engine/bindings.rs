use std::cell::UnsafeCell;

use crate::runtime::context::*;
use log::debug;

pub fn setup_bindings(isolate: &mut v8::Isolate) {
    isolate.set_host_import_module_dynamically_callback(host_import_module_dynamically_callback);
    isolate.set_host_initialize_import_meta_object_callback(
        host_initialize_import_meta_object_callback,
    );
}

fn host_import_module_dynamically_callback<'s>(
    scope: &mut v8::HandleScope<'s>,
    host_defined_options: v8::Local<'s, v8::Data>,
    resource_name: v8::Local<'s, v8::Value>,
    specifier: v8::Local<'s, v8::String>,
    import_assertions: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
    let context_id = match scope.get_slot::<ContextId>() {
        Some(key) => key.clone(),
        None => panic!("attempt to import from an unmanaged context"),
    };

    let specifier_str = specifier
        .to_string(scope)
        .unwrap()
        .to_rust_string_lossy(scope);

    debug!(
        "dyn_import specifier {} for engine {}",
        specifier_str, context_id,
    );

    let resolver = v8::PromiseResolver::new(scope).unwrap();
    let scope = UnsafeCell::new(scope);
    JsThreadContext::with_current_thread(move |ctx| {
        // SAFETY: we are in a thread context, so we can safely access the scope
        //         the value will not be dropped until after the closure is executed
        let scope = unsafe { scope.get().read() };
        assert_eq!(ctx.id, context_id);
        // let module_map = &mut ctx.module_map;

        // let module = module_map.get_by_name(&specifier_str);
        // if let Some(module) = module {
        //     let module = v8::Local::new(scope, module);
        //     resolver.resolve(scope, module);
        // }

        let message = v8::String::new(scope, "not implemented").unwrap().into();
        resolver.reject(scope, message);

        Some(resolver.get_promise(scope))
    })
}

extern "C" fn host_initialize_import_meta_object_callback(
    _context: v8::Local<v8::Context>,
    _module: v8::Local<v8::Module>,
    _meta: v8::Local<v8::Object>,
) {
    // TODO
}

pub fn throw_type_error(scope: &mut v8::HandleScope, message: impl AsRef<str>) {
    let message = v8::String::new(scope, message.as_ref()).unwrap();
    let exception = v8::Exception::type_error(scope, message);
    scope.throw_exception(exception);
}

#[allow(unused)]
fn import_meta_resolve(
    _scope: &mut v8::HandleScope,
    _args: v8::FunctionCallbackArguments,
    mut _rv: v8::ReturnValue,
) {
    // TODO
}
