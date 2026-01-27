{
  lib,
  glib,
  gtk4,
  gtk4-layer-shell,
  pkg-config,
  rustPlatform,
  rev ? "dirty",

}:
let
  cargoToml = lib.importTOML ./Cargo.toml;
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "hyprland-preview-share-picker";

  version = "${cargoToml.package.version}-${rev}";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.intersection (lib.fileset.fromSource (lib.sources.cleanSource ./.)) (
      lib.fileset.unions [
        ./lib
        ./src
        ./Cargo.toml
        ./Cargo.lock
        ./build.rs
        ./.gitmodules
      ]
    );
  };

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    glib
    gtk4
    gtk4-layer-shell
  ];

  strictDeps = true;
  cargoLock.lockFile = ./Cargo.lock;

  meta = {
    homepage = "https://github.com/WhySoBad/hyprland-preview-share-picker";
    license = lib.licenses.mit;
    maintainers = lib.maintainers.faukah;
  };
})
