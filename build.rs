//! Embeds the icon and the version information (generated here from `Cargo.toml`) into the Windows builds, and
//! the screensaver name and the manifest (`resources/windows.rc`) into the screensaver.

/// The `VERSIONINFO` resource: the Details tab of the file's properties.
#[cfg(windows)]
fn version_info(file_name: &str) -> String {
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    // `1.2.3-rc.1` -> 1,2,3,0: the numeric form has four numbers and no suffix.
    let numbers: Vec<&str> = version.split(['-', '+']).next().unwrap_or("").split('.').collect();
    let numeric = (0..4)
        .map(|i| {
            numbers
                .get(i)
                .and_then(|n| n.parse::<u16>().ok())
                .unwrap_or(0)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Open Scrantic contributors"
      VALUE "FileDescription", "Open Scrantic"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "OpenScrantic"
      VALUE "LegalCopyright", "MIT License"
      VALUE "OriginalFilename", "{file_name}"
      VALUE "ProductName", "Open Scrantic"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}

fn main() {
    // Resolve the DLLs this program imports from System32 only, never from the folder it sits in (which also
    // holds files from third parties). The linker flag exists only for MSVC.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/windows.rc");
    println!("cargo:rerun-if-changed=resources/windows.manifest");
    println!("cargo:rerun-if-changed=resources/icon.ico");
    // `embed-resource` is only built on Windows.
    #[cfg(windows)]
    {
        let dir =
            std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo")).join("resources");
        let saver = std::env::var_os("CARGO_FEATURE_SCREENSAVER").is_some();
        // The script is compiled from OUT_DIR, so the files it names need absolute paths.
        let path = |name: &str| dir.join(name).display().to_string().replace('\\', "/");
        let mut rc = format!("1 ICON \"{}\"\n", path("icon.ico"));
        if saver {
            rc += &std::fs::read_to_string(dir.join("windows.rc"))
                .expect("cannot read resources/windows.rc")
                .replace("\"windows.manifest\"", &format!("\"{}\"", path("windows.manifest")));
        }
        rc += &version_info(if saver { "OpenScrantic.scr" } else { "open-scrantic.exe" });
        let out = std::path::Path::new(&std::env::var_os("OUT_DIR").expect("set by cargo")).join("windows.rc");
        std::fs::write(&out, rc).expect("cannot write the generated resource script");
        embed_resource::compile(&out, embed_resource::NONE)
            .manifest_optional()
            .expect("cannot embed the Windows resources (RC.EXE of the Windows SDK is needed)");
    }
}
