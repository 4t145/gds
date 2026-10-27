{ callPackage, alsa-lib, procps }:
callPackage ../../package.nix {
  application = "agents";
  description = "Native GPUI workspace for Bas agents and conversations";
  extraBuildInputs = [ alsa-lib ];
  extraNativeCheckInputs = [ procps ];
  desktopItem = {
    name = "io.github.fourt145.GdsAgents";
    desktopName = "GDS Agents";
    genericName = "Agent Workspace";
    comment = "Manage Bas agents and conversations";
    exec = "gds-agents";
    icon = "applications-development";
    categories = [ "Development" "Utility" ];
  };
  postInstall = ''
    install -Dm644 src/bin/agents/NOTICE "$out/share/doc/gds-agents/NOTICE"
    install -Dm644 src/bin/agents/LICENSE "$out/share/licenses/gds-agents/LICENSE"
  '';
}
