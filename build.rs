//! Embeds the screensaver name and the manifest (`resources/windows.rc`) into the Windows screensaver.

fn main() {
    // Resolve the DLLs this program imports from System32 only, never from the folder it sits in (which also
    // holds files from third parties). The linker flag exists only for MSVC.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    println!("cargo:rerun-if-changed=resources/windows.rc");
    println!("cargo:rerun-if-changed=resources/windows.manifest");
    // Only the screensaver build has resources, and `embed-resource` is only built on Windows.
    #[cfg(windows)]
    if std::env::var_os("CARGO_FEATURE_SCREENSAVER").is_some() {
        embed_resource::compile("resources/windows.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("cannot embed the screensaver resources (RC.EXE of the Windows SDK is needed)");
    }
}
