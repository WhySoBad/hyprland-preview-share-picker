{
  mkShell,
  rustc,
  cargo,
  rustfmt,
  clippy,
  rust-analyzer-unwrapped,
  hyprland-preview-share-picker,
  rustPlatform,
}:
mkShell {
  name = "rust";
  inputsFrom = [ hyprland-preview-share-picker ];

  packages = [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer-unwrapped
  ];

  env.RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
