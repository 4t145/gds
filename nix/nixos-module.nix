flake:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.gds-files;
  desktopFile = "io.github.fourt145.GdsFiles.desktop";
  portal = pkgs.callPackage ./portal.nix { gds-files = cfg.package; };
in {
  options.programs.gds-files = {
    enable = lib.mkEnableOption "GDS Files";
    package = lib.mkOption {
      type = lib.types.package;
      default = flake.packages.${pkgs.stdenv.hostPlatform.system}.gds-files;
      defaultText = lib.literalExpression "inputs.gds.packages.\${pkgs.stdenv.hostPlatform.system}.gds-files";
      description = "GDS Files package to install.";
    };
    defaultFileManager = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Set the system default handler for inode/directory. User MIME settings take precedence.";
    };
    portal.enable = lib.mkEnableOption "GDS FileChooser portal with GTK fallback";
    portal.desktops = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "niri" ];
      description = "Desktop-specific portal configurations in addition to the common configuration.";
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    { environment.systemPackages = [ cfg.package ]; }
    (lib.mkIf cfg.defaultFileManager {
      xdg.mime.enable = true;
      xdg.mime.defaultApplications."inode/directory" = lib.mkDefault [ desktopFile ];
      xdg.mime.addedAssociations."inode/directory" = [ desktopFile ];
    })
    (lib.mkIf cfg.portal.enable {
      xdg.portal.enable = true;
      xdg.portal.extraPortals = [ portal pkgs.xdg-desktop-portal-gtk ];
      xdg.portal.config = lib.mkMerge [
        { common.default = lib.mkDefault [ "gtk" ]; }
        (lib.genAttrs ([ "common" ] ++ cfg.portal.desktops) (_: {
          "org.freedesktop.impl.portal.FileChooser" = lib.mkDefault [ "gds" ];
        }))
      ];
    })
  ]);
}
