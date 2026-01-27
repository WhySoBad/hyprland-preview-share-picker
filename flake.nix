{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
    self.submodules = true;
  };

  outputs =
    { nixpkgs, self }:
    let
      inherit (nixpkgs) lib;
      systems = lib.systems.flakeExposed;
      eachSystem = lib.genAttrs systems;
      pkgsFor = nixpkgs.legacyPackages;

      rev = self.shortRev or self.dirtyShortRev or "dirty";
    in
    {
      packages = eachSystem (system: {
        hyprland-preview-share-picker = pkgsFor.${system}.callPackage ./package.nix { inherit rev; };
        default = self.packages.${system}.hyprland-preview-share-picker;
      });

      devShells = eachSystem (system: {
        default = pkgsFor.${system}.callPackage ./shell.nix {
          inherit (self.packages.${system}) hyprland-preview-share-picker;
        };
      });

      apps = eachSystem (system: {
        default = {
          type = "app";
          program = "${
            self.packages.${system}.hyprland-preview-share-picker
          }/bin/hyprland-preview-share-picker";
        };
      });

    };
}
