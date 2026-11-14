{
  description = "@klercke's personal website";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks-nix = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.git-hooks-nix.flakeModule ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        {
          config,
          system,
          pkgs,
          ...
        }:
        let
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          pre-commit = {
            check.enable = false;
            settings = {
              package = pkgs.prek;
              hooks = {
                nixfmt.enable = true;
                reuse.enable = true;
                rustfmt = {
                  enable = true;
                  packageOverrides = {
                    cargo = toolchain;
                    rustfmt = toolchain;
                  };
                };
                clippy = {
                  enable = true;
                  packageOverrides = {
                    cargo = toolchain;
                    clippy = toolchain;
                  };
                  settings.denyWarnings = true;
                };
              };
            };
          };

          devShells.default = pkgs.mkShell {
            packages = [
              toolchain
              pkgs.zola
              pkgs.trunk
              pkgs.just
              pkgs.cargo-watch
              pkgs.prek
            ]
            ++ config.pre-commit.settings.enabledPackages;
            shellHook = config.pre-commit.installationScript;
          };

          formatter = pkgs.nixfmt;
        };
    };
}
