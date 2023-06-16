use log::debug;

pub fn host_import_module_dynamically_callback<'s>(
    scope: &mut v8::HandleScope<'s>,
    _host_defined_options: v8::Local<'s, v8::Data>,
    resource_name: v8::Local<'s, v8::Value>,
    specifier: v8::Local<'s, v8::String>,
    import_assertions: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
    // TODO
    None
    // // NOTE(bartlomieju): will crash for non-UTF-8 specifier
    // let specifier_str = specifier
    //     .to_string(scope)
    //     .unwrap()
    //     .to_rust_string_lossy(scope);
    // let referrer_name_str = resource_name
    //     .to_string(scope)
    //     .unwrap()
    //     .to_rust_string_lossy(scope);
    //
    // let resolver = v8::PromiseResolver::new(scope).unwrap();
    // let promise = resolver.get_promise(scope);
    //
    // let assertions = parse_import_assertions(
    //     scope,
    //     import_assertions,
    //     ImportAssertionsKind::DynamicImport,
    // );
    //
    // {
    //     let tc_scope = &mut v8::TryCatch::new(scope);
    //     validate_import_assertions(tc_scope, &assertions);
    //     if tc_scope.has_caught() {
    //         let e = tc_scope.exception().unwrap();
    //         resolver.reject(tc_scope, e);
    //     }
    // }
    // let asserted_module_type = get_asserted_module_type_from_assertions(&assertions);
    //
    // let resolver_handle = v8::Global::new(scope, resolver);
    // {
    //     let state_rc = JsRuntime::state_from(scope);
    //     let module_map_rc = JsRuntime::module_map_from(scope);
    //
    //     debug!(
    //         "dyn_import specifier {} referrer {} ",
    //         specifier_str, referrer_name_str
    //     );
    //     ModuleMap::load_dynamic_import(
    //         module_map_rc,
    //         &specifier_str,
    //         &referrer_name_str,
    //         asserted_module_type,
    //         resolver_handle,
    //     );
    //     state_rc.borrow_mut().notify_new_dynamic_import();
    // }
    // // Map errors from module resolution (not JS errors from module execution) to
    // // ones rethrown from this scope, so they include the call stack of the
    // // dynamic import site. Error objects without any stack frames are assumed to
    // // be module resolution errors, other exception values are left as they are.
    // let builder = v8::FunctionBuilder::new(catch_dynamic_import_promise_error);
    //
    // let map_err = v8::FunctionBuilder::<v8::Function>::build(builder, scope).unwrap();
    //
    // let promise = promise.catch(scope, map_err).unwrap();
    //
    // Some(promise)
}

pub extern "C" fn host_initialize_import_meta_object_callback(
    context: v8::Local<v8::Context>,
    module: v8::Local<v8::Module>,
    meta: v8::Local<v8::Object>,
) {
    // TODO
    // // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
    // let scope = &mut unsafe { v8::CallbackScope::new(context) };
    // let module_map_rc = JsRuntime::module_map_from(scope);
    // let module_map = module_map_rc.borrow();
    //
    // let module_global = v8::Global::new(scope, module);
    // let info = module_map
    //     .get_info(&module_global)
    //     .expect("Module not found");
    //
    // let url_key = v8::String::new_external_onebyte_static(scope, b"url").unwrap();
    // let url_val = info.name.v8(scope);
    // meta.create_data_property(scope, url_key.into(), url_val.into());
    //
    // let main_key = v8::String::new_external_onebyte_static(scope, b"main").unwrap();
    // let main_val = v8::Boolean::new(scope, info.main);
    // meta.create_data_property(scope, main_key.into(), main_val.into());
    //
    // let builder = v8::FunctionBuilder::new(import_meta_resolve).data(url_val.into());
    // let val = v8::FunctionBuilder::<v8::Function>::build(builder, scope).unwrap();
    // let resolve_key = v8::String::new_external_onebyte_static(scope, b"resolve").unwrap();
    // meta.set(scope, resolve_key.into(), val.into());
}

pub fn throw_type_error(scope: &mut v8::HandleScope, message: impl AsRef<str>) {
    let message = v8::String::new(scope, message.as_ref()).unwrap();
    let exception = v8::Exception::type_error(scope, message);
    scope.throw_exception(exception);
}

fn import_meta_resolve(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    // TODO
    // if args.length() > 1 {
    //     return throw_type_error(scope, "Invalid arguments");
    // }
    //
    // let maybe_arg_str = args.get(0).to_string(scope);
    // if maybe_arg_str.is_none() {
    //     return throw_type_error(scope, "Invalid arguments");
    // }
    // let specifier = maybe_arg_str.unwrap();
    // let referrer = {
    //     let url_prop = args.data();
    //     url_prop.to_rust_string_lossy(scope)
    // };
    // let module_map_rc = JsRuntime::module_map_from(scope);
    // let loader = module_map_rc.borrow().loader.clone();
    // let specifier_str = specifier.to_rust_string_lossy(scope);
    //
    // if specifier_str.starts_with("npm:") {
    //     throw_type_error(
    //         scope,
    //         "\"npm:\" specifiers are currently not supported in import.meta.resolve()",
    //     );
    //     return;
    // }
    //
    // match loader.resolve(&specifier_str, &referrer, ResolutionKind::DynamicImport) {
    //     Ok(resolved) => {
    //         let resolved_val = serde_v8::to_v8(scope, resolved.as_str()).unwrap();
    //         rv.set(resolved_val);
    //     }
    //     Err(err) => {
    //         throw_type_error(scope, &err.to_string());
    //     }
    // };
}
