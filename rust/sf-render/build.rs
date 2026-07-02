// Embed the SDL3 library dir as an rpath for TEST binaries only, so the
// nix-dev-shell libSDL3.so.0 (used by the offscreen GL tests) resolves at
// runtime without LD_LIBRARY_PATH. The library itself does not link SDL —
// sf-app owns the window and passes a glow::Context in.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if let Ok(out) = std::process::Command::new("pkg-config")
        .args(["--variable=libdir", "sdl3"])
        .output()
    {
        if out.status.success() {
            let libdir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !libdir.is_empty() {
                println!("cargo:rustc-link-arg-tests=-Wl,-rpath,{libdir}");
            }
        }
    }
}
