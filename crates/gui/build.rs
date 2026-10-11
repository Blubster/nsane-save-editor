//! Embeds the app icon and the version information into the Windows
//! executable. Other targets need nothing.

fn main() {
    println!("cargo::rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set(
                "ProductName",
                "Save editor for Crash Bandicoot N. Sane Trilogy",
            )
            .set(
                "FileDescription",
                "Save editor for Crash Bandicoot N. Sane Trilogy",
            )
            .set("InternalName", "nsane-save-editor")
            .set("OriginalFilename", "nsane-save-editor.exe")
            .set("LegalCopyright", "MIT license");
        res.compile().expect("can't embed the Windows resources");
    }
}
