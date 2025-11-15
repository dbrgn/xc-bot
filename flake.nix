{
  description = "A chat bot that notifies about new paragliding cross-country flights published on XContest";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-25.05";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    # Supported target systems
    allSystems = ["x86_64-linux"];

    # Helper to build a package for all supported systems above
    forAllSystems = f: nixpkgs.lib.genAttrs allSystems (system: f {pkgs = import nixpkgs {inherit system;};});

    mkPackage = pkgs: pkgs.callPackage ./package.nix {};
  in {
    # NixOS Module
    nixosModules.default = import ./nixos-module.nix self;

    # Package
    overlays.default = final: _prev: {xc-bot = mkPackage final;};
    packages = forAllSystems (
      {pkgs}: {
        default = mkPackage pkgs;
      }
    );

    # Tests
    checks = forAllSystems ({pkgs}: {
      test-module = pkgs.nixosTest (import ./nixos-tests/test-module.nix {
        inherit pkgs;
        modules = [self.nixosModules.default];
      });
    });
  };
}
