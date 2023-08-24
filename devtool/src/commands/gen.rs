use crate::cargo_container::CargoContainer;
use crate::cef::CefDist;
use anyhow::Result;
use std::path::PathBuf;

pub async fn command(cargo: CargoContainer<'_>) -> Result<()> {
    let cef_dist = CefDist::from(&cargo)?;
    println!("CEF version: {}", cef_dist.version);
    if !cef_dist.exists() {
        cef_dist.download().await?;
        cef_dist.extract()?;
    }

    // get the directory of the cef-sys package root
    let cef_sys_dir = cef_dist.package_dir.clone();

    let bindings = bindgen::Builder::default()
        .header(
            PathBuf::from(cef_sys_dir.clone())
                .join("cef.h")
                .to_str()
                .unwrap(),
        )
        .clang_arg(format!("-I{}", cef_dist.directory()))
        .allowlist_type("cef_main_args_t")
        .allowlist_function("cef_execute_process")
        .allowlist_type("cef_settings_t")
        .allowlist_function("cef_initialize")
        .allowlist_function("cef_run_message_loop")
        .allowlist_function("cef_shutdown")
        .allowlist_type("cef_string_t")
        .allowlist_function("cef_string_utf8_to_utf16")
        .allowlist_type("cef_base_ref_counted_t")
        .allowlist_type("cef_client_t")
        .allowlist_type("cef_life_span_handler_t")
        .allowlist_type("cef_display_handler_t")
        .allowlist_type("cef_browser_t")
        .allowlist_function("cef_browser_view_get_for_browser")
        .allowlist_function("cef_quit_message_loop")
        .allowlist_type("cef_frame_t")
        .allowlist_type("cef_load_handler_t")
        .allowlist_type("cef_app_t")
        .allowlist_type("cef_browser_process_handler_t")
        .allowlist_type("cef_browser_settings_t")
        .allowlist_type("cef_browser_view_delegate_t")
        .allowlist_type("cef_window_delegate_t")
        .allowlist_function("cef_browser_view_create")
        .allowlist_function("cef_window_create_top_level")
        .allowlist_type("cef_window_delegate_t")
        .allowlist_type("cef_browser_view_delegate_t")
        .allowlist_type("cef_view_delegate_t")
        .allowlist_type("cef_panel_delegate_t")
        .allowlist_type("cef_size_t")
        .allowlist_type("cef_render_handler_t")
        .allowlist_type("cef_text_input_mode_t")
        .allowlist_function("cef_enable_highdpi_support")
        .allowlist_function("cef_currently_on")
        .allowlist_type("cef_thread_id_t")
        .allowlist_function("cef_sandbox_info_create")
        .allowlist_function("cef_sandbox_info_destroy")
        .raw_line("#![allow(non_upper_case_globals)]")
        .raw_line("#![allow(non_camel_case_types)]")
        .raw_line("#![allow(non_snake_case)]")
        .derive_default(true)
        .layout_tests(false);

    let bindings = bindings.generate()?;

    let out_path = PathBuf::from(cef_sys_dir).join("src").join("lib.rs");
    bindings.write_to_file(out_path.clone())?;

    println!("Written bindings to {}", out_path.display());

    Ok(())
}
