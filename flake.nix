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
            "Cargo.toml"
            "Cargo.lock"
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

                # libclang_path is needed when not using flakebox
                # LIBCLANG_PATH = "${pkgs.libclang.lib}/lib/"; # (whisper-rs), bindgen needs this
                nativeBuildInputs = [
                  # tauri deps
                  pkgs.pkg-config
                  pkgs.gobject-introspection
                  pkgs.cargo-tauri
                  pkgs.nodejs
                  pnpm
                  pkgs.wrapGAppsHook # automatically wraps out binary to work with glib-networking

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
                  pkgs.glib-networking # glib-networking is a runtime deb of libsoup
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
            deps = craneLib.buildDepsOnly { };
            ${projectName} = craneLib.buildPackage {
              cargoArtifacts = deps;
              nativeBuildInputs = [ pnpm.configHook ];
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
          shellHook = ''
            # does what nativeBuildInputs[pkgs.wrapGAppsHook] do for glib-networking so it also
            # works in the dev shell
            export GIO_EXTRA_MODULES="$GIO_EXTRA_MODULES:${pkgs.glib-networking}/lib/gio/modules"
          '';
        };
      }
    );
}
