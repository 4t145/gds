{
  description = "GDS native desktop applications";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/02c087f3fe673c9e317f206a627cb6b910336dbe";

  outputs = { self, nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
    in {
      packages = forEachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          files = pkgs.callPackage ./nix/package.nix { };
        in {
          gds-files = files;
          gds-agents = pkgs.callPackage ./nix/package.nix { application = "agents"; };
          xdg-desktop-portal-gds = pkgs.callPackage ./nix/portal.nix { gds-files = files; };
          default = files;
        });

      apps = forEachSystem (system: {
        gds-files = {
          type = "app";
          program = "${self.packages.${system}.gds-files}/bin/gds-files";
        };
        gds-agents = {
          type = "app";
          program = "${self.packages.${system}.gds-agents}/bin/gds-agents";
        };
        default = {
          type = "app";
          meta.description = "GDS Files";
          program = "${self.packages.${system}.gds-files}/bin/gds-files";
        };
      });

      nixosModules = {
        gds-files = import ./nix/nixos-module.nix self;
        gds-agents = import ./nix/agents-module.nix self;
        default = self.nixosModules.gds-files;
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
