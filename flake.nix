{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      fenix,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        fenixChannel = fenix.packages.${system}.stable;
        fenixToolchain = (
          fenixChannel.withComponents [
            "rustc"
            "cargo"
            # "rustfmt"
            # "clippy"
            # "rust-analyzer"
            # "rust-src"
            # "llvm-tools-preview"
          ]
        );

        commonArgs = {
          nativeBuildInputs = with pkgs; [
            fenixToolchain
            nodePackages.pnpm

            pkg-config
            gobject-introspection
            cargo-tauri
            nodejs

            # whisper deps
            cmake
            # pkg-config
            shaderc
          ];
          buildInputs = with pkgs; [
            at-spi2-atk
            atkmm
            cairo
            gdk-pixbuf
            glib
            gtk3
            harfbuzz
            librsvg
            libsoup_2_4
            pango
            webkitgtk_4_0
            openssl

            alsa-lib

            piper-tts
            # whisper deps
            # alsa-lib
            vulkan-headers
            vulkan-loader
          ];
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib/"; # (whisper-rs), bindgen needs this
          shellHook = ''
            # export WEBKIT_DISABLE_COMPOSITING_MODE=1
          '';
        };
      in
      {
        devShells.default = pkgs.mkShell (
          commonArgs
          // {
            RUST_SRC_PATH = "${fenix.packages.${system}.stable.rust-src}/lib/rustlib/src/rust/library";

            RUST_LOG = "curses";

            packages = [
              fenixChannel.rust-analyzer
              fenixChannel.clippy
              fenixChannel.rustfmt
              pkgs.cargo-watch
              pkgs.typescript-language-server
            ];
          }
        );

        # packages.default = craneLib.buildPackage ( commonArgs // {
        #   src = craneLib.cleanCargoSource ./.;
        # });
      }
    );
}
