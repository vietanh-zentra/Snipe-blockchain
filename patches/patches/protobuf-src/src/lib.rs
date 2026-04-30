use std::path::PathBuf;

/// Returns the path to the system-installed `protoc` binary.
/// This shim replaces the original `protobuf-src` crate which tries to
/// compile protobuf from source using autotools (incompatible with Windows MSVC).
pub fn protoc() -> PathBuf {
    // Try to find protoc on PATH
    which_protoc().expect(
        "protoc not found on PATH. Install it via: winget install Google.Protobuf"
    )
}

fn which_protoc() -> Option<PathBuf> {
    // Check PROTOC env var first
    if let Ok(p) = std::env::var("PROTOC") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    // Search PATH
    let exe_name = if cfg!(windows) { "protoc.exe" } else { "protoc" };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(exe_name))
            .find(|p| p.exists())
    })
}
