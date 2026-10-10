use std::{io, path::Path};

fn ensure_official_icon_resources() -> io::Result<()> {
    // The checked-in files are generated from manager-icon.b64, the official
    // project source. Do not synthesize a fallback icon: that would silently
    // change the application's identity and can leave --no-bundle binaries
    // without the same Windows resource as bundled builds.
    let icon_dir = Path::new("icons");
    for file_name in ["manager-icon.b64", "icon.png", "icon.ico", "icon.icns"] {
        let path = icon_dir.join(file_name);
        println!("cargo:rerun-if-changed={}", path.display());
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("official application icon resource is missing: {}", path.display()),
            ));
        }
    }
    Ok(())
}

fn main() {
    ensure_official_icon_resources().expect("failed to validate official application icon resources");
    tauri_build::build()
}
