{
  description = "Star Fox SNES HD Remaster";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Build tools
            cmake
            pkg-config
            gcc

            # Runtime deps
            SDL2
            sdl3      # Rust app shell (sf-app) + Steam Controller 2 support
            libGL
            libGLU

            # Asset extraction
            python3
            python3Packages.pillow

            # Dev tools
            gdb
            valgrind
          ];

          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
              pkgs.SDL2
              pkgs.libGL
              pkgs.libGLU
            ]}:$LD_LIBRARY_PATH"
            echo "Star Fox HD dev shell ready"
            echo "  cmake -B build && cmake --build build"
            echo "  ./build/starfox-hd"
          '';
        };

        packages.default = pkgs.stdenv.mkDerivation {
          pname = "starfox-hd";
          version = "0.1.0";
          src = ./.;

          nativeBuildInputs = with pkgs; [ cmake pkg-config ];
          buildInputs = with pkgs; [ SDL2 libGL ];

          installPhase = ''
            mkdir -p $out/bin $out/share/starfox-hd/shaders
            cp starfox-hd $out/bin/
            cp -r shaders/* $out/share/starfox-hd/shaders/ 2>/dev/null || true
          '';
        };
      });
}
