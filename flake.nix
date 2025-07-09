{
  inputs = {
    # nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    flakebox.url = "github:rustshop/flakebox";
    flakebox.inputs.nixpkgs.follows = "nixpkgs";
    nixgl.url = "github:nix-community/nixGL";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      flakebox,
      nixgl,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        projectName = "curses";
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ nixgl.overlay ];
        };
        lib = pkgs.lib;
        pnpm = pkgs.pnpm_9;
        flakeboxLib = flakebox.lib.${system} {
          config = {
            github.ci.enable = false;
            semgrep.enable = false;
            flakebox.lint.enable = false;
            just = {
              enable = true;
              rules.watch.content = lib.mkForce ''
                # run and restart on changes
                watch:
                  #!/usr/bin/env bash
                  set -euo pipefail
                  if [ ! -f Cargo.toml ]; then
                    cd {{invocation_directory()}}
                  fi
                  env RUST_LOG=''${RUST_LOG:-debug} cargo tauri dev
              '';
              rules.build.content = lib.mkForce ''
                # run `cargo build` on everything
                build *ARGS="--no-bundle":
                  #!/usr/bin/env bash
                  set -euo pipefail
                  if [ ! -f Cargo.toml ]; then
                    cd {{invocation_directory()}}
                  fi
                  cargo tauri build {{ARGS}}
              '';
            };
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
            "dev-tools"
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

        more-alsa-plugins = pkgs.symlinkJoin {
          name = "more-alsa-plugins";
          paths = map (path: "${path}/lib/alsa-lib") (
            with pkgs;
            [
              pipewire
              alsa-plugins
            ]
          );
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
                ];
              }
            );
          in
          rec {
            deps = craneLib.buildDepsOnly { };
            dev-tools = craneLib.buildPackage {
              pname = "dev-tools";
              cargoArtifacts = deps;
              cargoExtraArgs = "--locked -p dev-tools";
            };
            curses = craneLib.buildPackage {
              cargoArtifacts = deps;
              nativeBuildInputs = [ pnpm.configHook ];
              pnpmDeps = pnpm.fetchDeps {
                pname = projectName;
                src = buildSrc;
                hash = "sha256-rrSWBKjLPQX+rErnqW/QCCokPKQKMjwhFH+93M1dEns=";
              };
              cargoBuildCommand = "cargo tauri build --no-bundle --";
              postInstall = ''
                wrapProgram $out/bin/curses \
                  ${pkgs.lib.optionalString pkgs.stdenv.isLinux "--set ALSA_PLUGIN_DIR ${more-alsa-plugins}"} \
                  --suffix PATH : ${pkgs.lib.makeBinPath [ pkgs.piper-tts ]}
              '';
            };
            nixGLCurses = pkgs.writeShellScriptBin "curses" ''
              ${pkgs.lib.getExe pkgs.nixgl.nixGLIntel} \
              ${pkgs.lib.getExe pkgs.nixgl.nixVulkanIntel} \
              ${curses}/bin/curses "$@"
            '';
          }
        );
      in
      {
        packages.default = multiBuild.nixGLCurses;
        packages.curses = multiBuild.curses;
        packages.dev-tools = multiBuild.dev-tools;
        devShells.lint = flakeboxLib.mkLintShell {
          packages = [
            pnpm
            pkgs.nodejs
          ];
        };
        devShells.default = flakeboxLib.mkDevShell {
          inputsFrom = [ multiBuild.curses ];
          packages = [
            pkgs.typescript-language-server
            pkgs.nixgl.nixGLIntel
            pkgs.nixgl.nixVulkanIntel
            pkgs.coreutils
            pkgs.piper-tts
          ];
          RUST_LOG = "curses";
          shellHook = ''
            # does what nativeBuildInputs[pkgs.wrapGAppsHook] do for glib-networking so it also
            # works in the dev shell
            export GIO_EXTRA_MODULES="$GIO_EXTRA_MODULES:${pkgs.glib-networking}/lib/gio/modules"
            ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''export ALSA_PLUGIN_DIR="${more-alsa-plugins}"''}

            # export the nixGL/vulkan paths automatically
            env_tempfile=$(mktemp XXXXXX-env.sh)
            nixGLIntel nixVulkanIntel ${lib.getExe pkgs.bash} -c "export -p" > "$env_tempfile"
            source "$env_tempfile"
            rm "$env_tempfile"
          '';
        };
      }
    );
}
