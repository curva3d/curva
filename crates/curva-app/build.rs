//! Puts Curva's icon into the Windows program. Elsewhere nothing is needed: the AppImage and the
//! macOS app carry the icon beside the program.

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icons/curva.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../packaging/icons/curva.ico");
        // Without the resource compiler the program still builds, with the system's icon.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Curva's icon was not added to the program: {error}");
        }
    }
}
