use anyhow::bail;
use anyhow::Result;
use rustc_hash::FxHashMap;
use std::path::PathBuf;

use super::bindings::throw_type_error;

pub type ModuleId = usize;
pub type ModuleLoadId = u32;
pub type ModuleName = String;
pub type ModuleCode = String;

#[derive(Debug)]
pub struct ModuleMap {
    handles: Vec<v8::Global<v8::Module>>,
    info: Vec<ModuleInfo>,
    load_base_path: PathBuf,
    by_name: FxHashMap<ModuleName, ModuleId>,

    next_load_id: ModuleLoadId,
    dynamic_import_map: FxHashMap<ModuleLoadId, v8::Global<v8::PromiseResolver>>,
}

#[derive(Debug, PartialEq)]
pub struct ModuleInfo {
    pub id: ModuleId,
    pub name: ModuleName,
    pub requests: Vec<ModuleRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleRequest {
    pub specifier: String,
}

impl ModuleMap {
    pub fn new(load_base_path: PathBuf) -> Self {
        Self {
            handles: vec![],
            info: vec![],
            load_base_path,
            by_name: FxHashMap::default(),
            next_load_id: 1,
            dynamic_import_map: FxHashMap::default(),
        }
    }

    pub fn get_handle(&self, id: ModuleId) -> Option<v8::Global<v8::Module>> {
        self.handles.get(id).cloned()
    }

    pub fn get_by_name(&self, specifier: &str) -> Option<v8::Global<v8::Module>> {
        self.by_name
            .get(specifier)
            .and_then(|id| self.get_handle(*id))
    }

    pub fn get_info(&self, global: &v8::Global<v8::Module>) -> Option<&ModuleInfo> {
        if let Some(id) = self.handles.iter().position(|module| module == global) {
            return self.info.get(id);
        }

        None
    }

    pub fn insert_module(
        &mut self,
        name: ModuleName,
        handle: v8::Global<v8::Module>,
        requests: Vec<ModuleRequest>,
    ) -> ModuleId {
        let id = self.handles.len();
        self.by_name.insert(name.clone(), id);
        self.handles.push(handle);
        self.info.push(ModuleInfo { id, name, requests });

        id
    }

    pub fn insert_alias(&mut self, specifier: ModuleName, target: ModuleId) {
        self.by_name.insert(specifier, target);
    }

    fn resolve_specifier(&self, specifier: &str, referrer: &str) -> ModuleName {
        let mut path = PathBuf::from(referrer);
        path.pop();
        path.push(specifier);
        path.to_string_lossy().into()
    }

    fn create_origin<'s>(
        scope: &mut v8::HandleScope<'s>,
        name: &str,
        source_map_url: Option<&str>,
    ) -> v8::ScriptOrigin<'s> {
        let name = v8::String::new(scope, name).unwrap().into();
        let source_map_url = v8::String::new(scope, source_map_url.unwrap_or(""))
            .unwrap()
            .into();

        let origin = v8::ScriptOrigin::new(
            scope,
            name,
            0,
            0,
            false,
            0,
            source_map_url,
            true,
            false,
            true,
        );

        origin
    }

    pub fn new_es_module(
        &mut self,
        scope: &mut v8::HandleScope,
        name: ModuleName,
        source: ModuleCode,
    ) -> Result<ModuleId> {
        // let path = self.load_base_path.join(specifier);
        // let source = std::fs::read_to_string(&path).unwrap();
        let source = v8::String::new(scope, &source).unwrap();

        let origin = Self::create_origin(scope, &name, None);
        let source = v8::script_compiler::Source::new(source, Some(&origin));

        // enter a new try-catch scope to catch load errors
        let tc_scope = &mut v8::TryCatch::new(scope);
        let maybe_module = v8::script_compiler::compile_module(tc_scope, source);

        if tc_scope.has_caught() {
            // error compiling, bail
            assert!(maybe_module.is_none());
            let exception = tc_scope.exception().unwrap();
            let message = exception.to_string(tc_scope).unwrap();
            let message = message.to_rust_string_lossy(tc_scope);
            bail!("TODO: tc_scope caught during module load: {}", message);
            // return Err(JsError::Value(JsValue::from_v8_value()));
        }

        // module compiled OK, check it out
        let module = maybe_module.unwrap();
        let mut requests: Vec<ModuleRequest> = vec![];
        let module_requests = module.get_module_requests();
        for i in 0..module_requests.length() {
            let request =
                v8::Local::<v8::ModuleRequest>::try_from(module_requests.get(tc_scope, i).unwrap())
                    .unwrap();

            let import_spec = request.get_specifier().to_rust_string_lossy(tc_scope);

            requests.push(ModuleRequest {
                specifier: self.resolve_specifier(&import_spec, "."),
            });
        }

        let handle = v8::Global::new(tc_scope, module);
        let id = self.insert_module(name, handle, requests);
        Ok(id)
    }

    pub fn instantiate_module(
        &mut self,
        scope: &mut v8::HandleScope,
        id: ModuleId,
    ) -> Result<(), v8::Global<v8::Value>> {
        let tc_scope = &mut v8::TryCatch::new(scope);

        let module = self
            .get_handle(id)
            .map(|handle| v8::Local::new(tc_scope, handle))
            .expect("ModuleInfo not found");

        if module.get_status() == v8::ModuleStatus::Errored {
            return Err(v8::Global::new(tc_scope, module.get_exception()));
        }

        tc_scope.set_slot(self as *const _);
        let instantiate_result = module.instantiate_module(tc_scope, Self::module_resolve_callback);
        tc_scope.remove_slot::<*const Self>();
        if instantiate_result.is_none() {
            let exception = tc_scope.exception().unwrap();
            return Err(v8::Global::new(tc_scope, exception));
        }

        Ok(())
    }

    /// Called by V8 during `JsRuntime::instantiate_module`. This is only used internally, so we use the Isolate's annex
    /// to propagate a &Self.
    fn module_resolve_callback<'s>(
        context: v8::Local<'s, v8::Context>,
        specifier: v8::Local<'s, v8::String>,
        import_assertions: v8::Local<'s, v8::FixedArray>,
        referrer: v8::Local<'s, v8::Module>,
    ) -> Option<v8::Local<'s, v8::Module>> {
        // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
        let scope = &mut unsafe { v8::CallbackScope::new(context) };

        // SAFETY: We retrieve the pointer from the slot, having just set it a few stack frames up
        let module_map = unsafe { scope.get_slot::<*const Self>().unwrap().as_ref().unwrap() };

        let referrer_global = v8::Global::new(scope, referrer);

        let referrer_info = module_map
            .get_info(&referrer_global)
            .expect("ModuleInfo not found");
        let referrer_name = referrer_info.name.as_str();

        let specifier_str = specifier.to_rust_string_lossy(scope);

        let maybe_module = module_map.resolve_callback(scope, &specifier_str, referrer_name);
        if let Some(module) = maybe_module {
            return Some(module);
        }

        let msg = format!(r#"Cannot resolve module "{specifier_str}" from "{referrer_name}""#);
        throw_type_error(scope, msg);
        None
    }

    /// Called by `module_resolve_callback` during module instantiation.
    fn resolve_callback<'s>(
        &self,
        scope: &mut v8::HandleScope<'s>,
        specifier: &str,
        referrer: &str,
    ) -> Option<v8::Local<'s, v8::Module>> {
        let module = self
            .get_by_name(specifier)
            .expect("Module should have already been resolved");
        let module = v8::Local::new(scope, &module);
        Some(module)
    }
}
