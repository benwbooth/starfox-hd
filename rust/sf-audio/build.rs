//! Compiles the bundled snes_spc C++ emulator (same sources the C oracle
//! links: ../src/snes_spc/*.cpp) into a static lib without external crate
//! dependencies: shells out to $CXX directly.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const SOURCES: &[&str] = &[
    "SNES_SPC.cpp",
    "SNES_SPC_misc.cpp",
    "SNES_SPC_state.cpp",
    "SPC_DSP.cpp",
    "SPC_Filter.cpp",
    "dsp.cpp",
    "spc.cpp",
];

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let spc_dir = manifest.join("../../src/snes_spc");
    let cxx = env::var("CXX").unwrap_or_else(|_| "c++".to_string());

    let mut objects = Vec::new();
    for src in SOURCES {
        let src_path = spc_dir.join(src);
        let obj = out_dir.join(format!("{src}.o"));
        // -DBLARGG_BUILD_DLL matches the C build (CMakeLists.txt snes_spc
        // target): it makes debug_printf a no-op in blargg_source.h.
        let status = Command::new(&cxx)
            .args(["-O2", "-fPIC", "-w", "-DBLARGG_BUILD_DLL", "-c"])
            .arg(&src_path)
            .arg("-o")
            .arg(&obj)
            .status()
            .expect("run C++ compiler");
        assert!(status.success(), "compile {src}");
        objects.push(obj);
        println!("cargo:rerun-if-changed={}", src_path.display());
    }

    let lib = out_dir.join("libsnes_spc_rs.a");
    let _ = std::fs::remove_file(&lib);
    let status = Command::new("ar")
        .arg("crs")
        .arg(&lib)
        .args(&objects)
        .status()
        .expect("run ar");
    assert!(status.success(), "archive libsnes_spc_rs.a");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=snes_spc_rs");
    // snes_spc is C++; link the C++ runtime. On Nix the runtime dir is not
    // on the default search path, so resolve it from the compiler and pin
    // an rpath for test/binary execution.
    let libpath = Command::new(&cxx)
        .arg("-print-file-name=libstdc++.so")
        .output()
        .expect("query libstdc++ path");
    let libpath = String::from_utf8_lossy(&libpath.stdout);
    if let Some(dir) = PathBuf::from(libpath.trim()).parent() {
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
