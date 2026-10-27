{ lib, pkgs, rustPlatform, makeWrapper, makeDesktopItem, copyDesktopItems, pkg-config, cmake, application ? "files" }:
let
  isAgents = application == "agents";
  binary = "gds-${application}";
  appId = if isAgents then "io.github.fourt145.GdsAgents" else "io.github.fourt145.GdsFiles";
  libraries = with pkgs; [
    wayland libxkbcommon fontconfig freetype openssl zstd zlib
    libGL vulkan-loader dbus libx11 libxcb
  ] ++ lib.optionals isAgents [ pkgs.alsa-lib ];
in
rustPlatform.buildRustPackage {
  pname = binary;
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../LICENSE
      (lib.fileset.difference ../src (lib.fileset.unions [
        ../src/bin/files/portal.py
        ../src/bin/files/portal
      ]))
    ];
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes."gds-uim-0.1.0" = "sha256-yVojxpvvejy2NBKgcKsffBMqrPqe6YOJ94vtjCUz7dQ=";
  };

  nativeBuildInputs = [ pkg-config cmake makeWrapper copyDesktopItems rustPlatform.bindgenHook ];
  nativeCheckInputs = [ pkgs.dbus ] ++ lib.optionals isAgents [ pkgs.procps ];
  buildInputs = libraries;
  dontUseCmakeConfigure = true;
  cargoBuildFlags = [ "--bin" binary ];
  buildFeatures = lib.optional isAgents "agents";
  checkFeatures = lib.optional isAgents "agents";
  cargoTestFlags = lib.optionals isAgents [ "--bin" binary "--lib" ];
  # Nix's Linux sandbox rejects chmod with setuid/setgid bits.
  checkFlags = [ "--skip=directory::tests::permissions_owner_group_and_access_time_come_from_real_metadata" ];

  desktopItems = [ (makeDesktopItem {
    name = appId;
    desktopName = if isAgents then "GDS Agents" else "GDS Files";
    genericName = if isAgents then "Agent Workspace" else "File Manager";
    comment = if isAgents then "Manage Bas agents and conversations" else "Browse local files and folders";
    exec = if isAgents then binary else "gds-files -- %f";
    icon = if isAgents then "applications-development" else "system-file-manager";
    terminal = false;
    categories = if isAgents then [ "Development" "Utility" ] else [ "System" "FileTools" "FileManager" ];
    mimeTypes = lib.optionals (!isAgents) [ "inode/directory" ];
    startupWMClass = appId;
  }) ];

  postInstall = if isAgents then ''
    install -Dm644 src/bin/agents/NOTICE "$out/share/doc/gds-agents/NOTICE"
    install -Dm644 src/bin/agents/LICENSE "$out/share/licenses/gds-agents/LICENSE"
  '' else ''
    install -Dm644 ${./niri.kdl} "$out/share/gds-files/niri.kdl"
  '';

  postFixup = ''
    wrapProgram "$out/bin/${binary}" \
      --prefix PATH : ${lib.makeBinPath [ pkgs.xdg-utils pkgs.xdg-terminal-exec ]} \
      --prefix LD_LIBRARY_PATH : "/run/opengl-driver/lib:${lib.makeLibraryPath libraries}"
  '';

  meta = {
    description = if isAgents then "Native GPUI workspace for Bas agents and conversations" else "Native GPUI file manager and single-file picker";
    homepage = "https://github.com/4t145/gds";
    mainProgram = binary;
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
  };
}
