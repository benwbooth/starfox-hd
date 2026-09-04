{
  description = "Star Fox SNES HD Remaster (Rust)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = inputs@{ self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        # Shared runtime libraries. The engine (incl. the SPC-700 core) is
        # pure Rust now; SDL3 is still C, and GL/libstdc++ are needed at
        # runtime for the windowed binary.
        runtimeLibs = with pkgs; [
          SDL2      # legacy audio device fallback
          sdl3      # Rust app shell (sf-app) window + input + audio
          libGL
          libGLU
          vulkan-loader     # wgpu Vulkan backend (surface via VK_KHR_wayland_surface)
          wayland           # native Wayland windowing (no Xwayland needed)
          libdecor          # SDL3 client-side window decorations on Wayland
          stdenv.cc.cc.lib  # libstdc++ (SDL3 / GL drivers)
          zlib              # rustup rustc's dynamically linked LLVM runtime
        ];
        # The independent Mesen oracle needs these even in its headless test
        # runner. Keep them in the development environment only.
        oracleRuntimeLibs = with pkgs; [ libx11 icu ];
      in
      {
        devShells.default = pkgs.mkShell {
          FLAKE_INPUTS = builtins.concatStringsSep ":" (
            map (input: input.outPath) (
              builtins.attrValues (builtins.removeAttrs inputs [ "self" ])
            )
          );

          buildInputs = with pkgs; [
            # Build tools
            pkg-config
            gcc          # cc: Rust's default linker driver

            # Runtime deps
            SDL2
            sdl3
            libGL
            libGLU

            # Asset extraction / codegen (tools/*.py)
            python3
            python3Packages.pillow

            # Lossless retail-ROM reconstruction. WLA-DX provides independent
            # 65C816 and Super FX assemblers for the verification toolchain;
            # neither assembler is linked into the shipping Rust port.
            wla-dx

            # Dev tools
            gdb
            valgrind
          ];

          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath (runtimeLibs ++ oracleRuntimeLibs)}:$LD_LIBRARY_PATH"
            echo "Star Fox HD dev shell ready (Rust)"
            echo "  cd rust && cargo build"
            echo "  ./scripts/run.sh        # launch from the repo root"
          '';
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "starfox-hd-rs";
          version = "0.1.0";
          src = ./rust;
          cargoLock.lockFile = ./rust/Cargo.lock;

          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = runtimeLibs;

          # Only the integration binary crate is a product.
          cargoBuildFlags = [ "-p" "sf-app" ];
          doCheck = false;

          postInstall = ''
            mkdir -p $out/share/starfox-hd/shaders
            cp sf-render/shaders/*.glsl $out/share/starfox-hd/shaders/ 2>/dev/null || true
          '';
        };
      });
}
