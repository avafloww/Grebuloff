use anyhow::anyhow;
use anyhow::Result;
use cargo::{
    core::{
        compiler::{BuildConfig, Compilation, Executor},
        resolver::CliFeatures,
        Workspace,
    },
    ops::{CompileFilter, Packages},
    util::command_prelude::CompileMode,
    CargoResult, Config,
};
use cargo_util::ProcessBuilder;
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct CargoContainer<'a> {
    pub config: &'a Config,
    pub workspace: Workspace<'a>,
}

pub struct CompileOptions {
    pub packages: Vec<String>,
    pub env: HashMap<String, String>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            packages: vec![],
            env: HashMap::new(),
        }
    }
}

static CURRENT_EXECUTOR: Mutex<Option<Arc<dyn Executor>>> = Mutex::new(None);

impl<'a> CargoContainer<'a> {
    pub fn new(config: &'a Config) -> Result<Self> {
        let ws_root = Self::find_workspace_root_manifest().expect("failed to find workspace root");
        let ws = Workspace::new(&ws_root, &config)?;

        Ok(Self {
            workspace: ws,
            config,
        })
    }

    fn find_workspace_root_manifest() -> Result<PathBuf> {
        // traverse up the directory tree until we find a Cargo.toml
        let mut current_dir = env::current_exe()?
            .parent()
            .ok_or(anyhow!("failed to get exe directory"))?
            .to_path_buf();
        loop {
            let manifest_path = current_dir.join("Cargo.toml");
            if manifest_path.exists() {
                return Ok(manifest_path);
            }

            current_dir = current_dir
                .parent()
                .ok_or(anyhow!("failed to find Cargo.toml"))?
                .to_path_buf();
        }
    }

    pub fn compile(&self, options: CompileOptions) -> CargoResult<Compilation<'a>> {
        let opts = cargo::ops::CompileOptions {
            spec: if options.packages.len() > 0 {
                Packages::Packages(options.packages.clone())
            } else {
                Packages::Default
            },
            build_config: BuildConfig::new(&self.config, None, false, &[], CompileMode::Build)?,
            cli_features: CliFeatures::new_all(false),
            filter: CompileFilter::Default {
                required_features_filterable: false,
            },
            target_rustdoc_args: None,
            target_rustc_args: None,
            target_rustc_crate_types: None,
            rustdoc_document_private_items: false,
            honor_rust_version: true,
        };

        // ensure that only one of this function is running at a time
        let mut mutex = CURRENT_EXECUTOR.lock().unwrap();
        assert!(mutex.is_none());

        // create a new executor and make it 'static
        let executor: Arc<CargoExecutor> = Arc::new(options.into());
        *mutex = Some(executor);

        // execute the compile
        let result = cargo::ops::compile_with_exec(&self.workspace, &opts, &mutex.clone().unwrap());

        // clear the mutex
        *mutex = None;

        result
    }
}

#[derive(Clone)]
struct CargoExecutor {
    extra_env: HashMap<String, String>,
}

impl Executor for CargoExecutor {
    fn exec(
        &self,
        cmd: &ProcessBuilder,
        _id: cargo::core::PackageId,
        _target: &cargo::core::Target,
        _mode: CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        let mut owned = cmd.to_owned();

        for (key, val) in &self.extra_env {
            owned.env(key, val);
        }

        owned
            .exec_with_streaming(on_stdout_line, on_stderr_line, false)
            .map(drop)
    }
}

impl From<CompileOptions> for CargoExecutor {
    fn from(options: CompileOptions) -> Self {
        CargoExecutor {
            extra_env: options.env,
        }
    }
}
