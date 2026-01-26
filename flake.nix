{
  description = "An alternative share picker for hyprland with window and monitor previews";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
        ];

        buildInputs = with pkgs; [
          # GTK4 dependencies
          gtk4
          gtk4-layer-shell
          glib
          pango
          cairo
          gdk-pixbuf

          # Wayland dependencies
          wayland
          wayland-protocols
          wayland-scanner

          # System dependencies
          openssl
          dbus

          # Image handling
          libpng
          libjpeg_turbo

          # Other dependencies
          libxkbcommon
        ];
      in
      {
        # Default package
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "hyprland-preview-share-picker";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;

          # Environment variables for development
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "🦀 Rust development environment ready!"
            echo "GTK4 and Wayland dependencies are available"
          '';
        };

        # Application package (for easier installation)
        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/hyprland-preview-share-picker";
        };
      }
    );
}
