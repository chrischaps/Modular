//! Puts the app icon (assets/icon, drawn by tools/icon/make_icon.py) into the
//! Windows executable, so Explorer, the taskbar and shortcuts show it. A
//! build without the resource compiler still works, only without the icon.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/soba.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon/soba.ico");
        if let Err(e) = resource.compile() {
            println!("cargo:warning=the executable has no icon: {e}");
        }
    }
}
