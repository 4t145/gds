{ lib, pkgs, rustPlatform, makeWrapper, makeDesktopItem, copyDesktopItems, pkg-config, cmake }:
let
  libraries = with pkgs; [
    wayland libxkbcommon fontconfig freetype openssl zstd zlib
    libGL vulkan-loader dbus libx11 libxcb
  ];
in
rustPlatform.buildRustPackage {
  pname = "gds-files";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
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
  buildInputs = libraries;
  dontUseCmakeConfigure = true;
  cargoBuildFlags = [ "--bin" "gds-files" ];
  # Nix's Linux sandbox rejects chmod with setuid/setgid bits.
  checkFlags = [ "--skip=directory::tests::permissions_owner_group_and_access_time_come_from_real_metadata" ];

  desktopItems = [ (makeDesktopItem {
    name = "io.github.fourt145.GdsFiles";
    desktopName = "GDS Files";
    genericName = "File Manager";
    comment = "Browse local files and folders";
    exec = "gds-files -- %f";
    icon = "system-file-manager";
    terminal = false;
    categories = [ "System" "FileTools" "FileManager" ];
    mimeTypes = [ "inode/directory" ];
    startupWMClass = "io.github.fourt145.GdsFiles";
  }) ];

  postInstall = ''
    install -Dm644 ${./niri.kdl} "$out/share/gds-files/niri.kdl"
  '';

  postFixup = ''
    wrapProgram "$out/bin/gds-files" \
      --prefix PATH : ${lib.makeBinPath [ pkgs.xdg-utils pkgs.xdg-terminal-exec ]} \
      --prefix LD_LIBRARY_PATH : "/run/opengl-driver/lib:${lib.makeLibraryPath libraries}"
  '';

  meta = {
    description = "Native GPUI file manager and single-file picker";
    homepage = "https://github.com/4t145/gds";
    mainProgram = "gds-files";
    platforms = lib.platforms.linux;
  };
}
