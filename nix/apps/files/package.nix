{ callPackage }:
callPackage ../../package.nix {
  application = "files";
  description = "Native GPUI file manager and single-file picker";
  # Nix's Linux sandbox rejects chmod with setuid/setgid bits.
  checkFlags = [ "--skip=directory::tests::permissions_owner_group_and_access_time_come_from_real_metadata" ];
  desktopItem = {
    name = "io.github.fourt145.GdsFiles";
    desktopName = "GDS Files";
    genericName = "File Manager";
    comment = "Browse local files and folders";
    exec = "gds-files -- %f";
    icon = "system-file-manager";
    categories = [ "System" "FileTools" "FileManager" ];
    mimeTypes = [ "inode/directory" ];
  };
  postInstall = ''
    install -Dm644 ${./niri.kdl} "$out/share/gds-files/niri.kdl"
  '';
}
