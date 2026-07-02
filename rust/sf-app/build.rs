// Embed the SDL3 library dir as an rpath so the nix-dev-shell libSDL3.so.0
// resolves at runtime without LD_LIBRARY_PATH. Same pattern as
// rust/sf-render/build.rs, but sf-app links SDL for real (binary + tests).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if let Ok(out) = std::process::Command::new("pkg-config")
        .args(["--variable=libdir", "sdl3"])
        .output()
    {
        if out.status.success() {
            let libdir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !libdir.is_empty() {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir}");
            }
        }
    }
}
