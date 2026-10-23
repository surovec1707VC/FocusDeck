fn main() {
    // The app manifest is embedded by the linker instead of tauri-build's
    // resource, so the test harness gets it too (otherwise tests that link
    // the dialog plugin fail with STATUS_ENTRYPOINT_NOT_FOUND).
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri-build");

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        // No second, linker-generated UAC section: the default asInvoker is fine.
        println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
    }
}
