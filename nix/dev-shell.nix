{ pkgs }:
let
  libraries = with pkgs; [
    wayland libxkbcommon fontconfig freetype openssl zstd zlib
    libGL vulkan-loader dbus libx11 libxcb alsa-lib
  ];
in
pkgs.mkShell {
  packages = with pkgs; [
    cargo rustc rustfmt clippy just pkg-config clang lld cmake python3 ncurses
  ];
  buildInputs = libraries;
  LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
  shellHook = ''
    export LD_LIBRARY_PATH="/run/opengl-driver/lib:${pkgs.lib.makeLibraryPath libraries}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export TERMINFO_DIRS="''${TERMINFO_DIRS:+$TERMINFO_DIRS:}${pkgs.ncurses}/share/terminfo"
  '';
}
