// Embed Windows executable icon when building on Windows.
// Place icon.ico in mandelbrust-app/ (next to Cargo.toml). If present and
// embedding fails, the build fails with a clear error (e.g. install rc.exe).

fn main() {
    #[cfg(target_os = "windows")]
    {
        let icon_path = std::path::Path::new("icon.ico");
        if icon_path.exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path.to_str().expect("icon path"));
            if let Err(e) = res.compile() {
                panic!(
                    "Failed to embed icon from icon.ico: {}. \
                     On Windows you need a resource compiler (e.g. rc.exe from Visual Studio Build Tools) in PATH.",
                    e
                );
            }
        }
    }
}
