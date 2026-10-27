{
  description = "GDS native desktop applications";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/02c087f3fe673c9e317f206a627cb6b910336dbe";

  outputs = { self, nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
      applications = builtins.attrNames (nixpkgs.lib.filterAttrs
        (_: type: type == "directory") (builtins.readDir ./nix/apps));
      applicationNames = map (name: "gds-${name}") applications;
    in {
      packages = forEachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          packages = nixpkgs.lib.genAttrs applicationNames (name:
            pkgs.callPackage (./nix/apps + "/${nixpkgs.lib.removePrefix "gds-" name}/package.nix") { });
          suite = pkgs.buildEnv {
            name = "gds";
            paths = builtins.attrValues packages;
            meta.description = "GDS native desktop application suite";
          };
        in packages // {
          gds = suite;
          xdg-desktop-portal-gds = pkgs.callPackage ./nix/apps/files/portal.nix {
            gds-files = packages.gds-files;
          };
          default = suite;
        });

      apps = forEachSystem (system:
        nixpkgs.lib.genAttrs applicationNames (name: {
          type = "app";
          meta.description = self.packages.${system}.${name}.meta.description;
          program = "${self.packages.${system}.${name}}/bin/${name}";
        }));

      nixosModules = nixpkgs.lib.genAttrs applicationNames (name:
        import (./nix/apps + "/${nixpkgs.lib.removePrefix "gds-" name}/module.nix") self
      ) // {
        default = {
          imports = map (name: self.nixosModules.${name}) applicationNames;
        };
      };

      checks = forEachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          python = pkgs.python3.withPackages (ps: [ ps.dbus-next ]);
        in {
          portal = pkgs.runCommand "gds-portal-tests" {
            nativeBuildInputs = [ python pkgs.dbus ];
          } ''
            export PYTHONDONTWRITEBYTECODE=1
            dbus-run-session --config-file=${pkgs.dbus}/share/dbus-1/session.conf \
              -- python ${./src/bin/files}/portal/tests.py
            touch "$out"
          '';
        });

      devShells = forEachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in {
          default = import ./nix/dev-shell.nix { inherit pkgs; };
        });
    };
}
