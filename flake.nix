{
  inputs = {
    # nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    flakebox.url = "github:rustshop/flakebox";
    flakebox.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      flakebox,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        projectName = "curses";
        pkgs = nixpkgs.legacyPackages.${system};
        pnpm = pkgs.pnpm_9;
        flakeboxLib = flakebox.lib.${system} {
          config = {
            github.ci.enable = false;
            semgrep.enable = false;
          };
        };

        buildSrc = flakeboxLib.filterSubPaths {
          root = builtins.path {
            name = projectName;
            path = ./.;
          };
          paths = [
            "src-tauri"
            "src"
            "public"
            "tests"
            "index.html"
            "package.json"
            "playwright.config.ts"
            "pnpm-lock.yaml"
            "postcss.config.cjs"
            "tailwind.config.cjs"
            "tsconfig.json"
            "tsconfig.node.json"
            "vite.config.ts"
          ];
        };

        multiBuild = (flakeboxLib.craneMultiBuild { }) (
          craneLib':
          let
            craneLib = (
              craneLib'.overrideArgs {
                pname = projectName;
                src = buildSrc;
                cargoLock = ./src-tauri/Cargo.lock;
                cargoToml = ./src-tauri/Cargo.toml;
                postConfigure = ''
                  # cargo lock is filtered out and a stripped version is placed at src root
                  mv Cargo.lock src-tauri/
                  cd src-tauri
                '';

                # libclang_path is needed when not using flakebox
                # LIBCLANG_PATH = "${pkgs.libclang.lib}/lib/"; # (whisper-rs), bindgen needs this
                nativeBuildInputs = [
                  # tauri deps
                  pkgs.pkg-config
                  pkgs.gobject-introspection
                  pkgs.cargo-tauri
                  pkgs.nodejs
                  pnpm

                  # whisper extra deps
                  pkgs.cmake
                  pkgs.shaderc
                ];
                buildInputs = [
                  # tauri deps
                  pkgs.at-spi2-atk
                  pkgs.atkmm
                  pkgs.cairo
                  pkgs.gdk-pixbuf
                  pkgs.glib
                  pkgs.gtk3
                  pkgs.harfbuzz
                  pkgs.librsvg
                  pkgs.libsoup_3
                  pkgs.pango
                  pkgs.webkitgtk_4_1
                  pkgs.openssl

                  # whisper extra deps
                  pkgs.vulkan-headers
                  pkgs.vulkan-loader

                  pkgs.alsa-lib

                  # piper binary pre-shipped
                  pkgs.piper-tts
                ];
              }
            );
          in
          rec {
            deps = craneLib.buildDepsOnly { src = buildSrc; };
            ${projectName} = craneLib.buildPackage {
              cargoArtifacts = deps;
              nativeBuildInputs = [ pnpm.configHook ];
              preBuild = ''
                mv ../target target
              '';
              pnpmDeps = pnpm.fetchDeps {
                pname = projectName;
                src = buildSrc;
                hash = "sha256-pdHVo9iqTSPOjaeMxndFXS6vNhg7+EHLTGnImCgoKpQ=";
              };
              cargoBuildCommand = "cargo tauri build --no-bundle --";
            };
          }
        );
      in
      {
        packages.default = multiBuild.${projectName};
        legacyPackages = multiBuild;
        devShells = flakeboxLib.mkShells {
          inputsFrom = [ multiBuild.${projectName} ];
          packages = [
            pkgs.typescript-language-server
          ];
          RUST_LOG = "curses";
        };
      }
    );
}
