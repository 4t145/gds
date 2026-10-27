{ lib, pkgs, rustPlatform, makeWrapper, makeDesktopItem, copyDesktopItems, pkg-config, cmake
, application, desktopItem, description, postInstall ? "", checkFlags ? [ ]
, extraBuildInputs ? [ ], extraNativeCheckInputs ? [ ]
}:
let
  binary = "gds-${application}";
  manifest = builtins.fromTOML (builtins.readFile ../Cargo.toml);
  target = lib.findFirst (target: target.name == binary)
    (throw "Cargo.toml does not declare ${binary}") manifest.bin;
  features = target.required-features or [ ];
  libraries = with pkgs; [
    wayland libxkbcommon fontconfig freetype openssl zstd zlib
    libGL vulkan-loader dbus libx11 libxcb
  ] ++ extraBuildInputs;
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
      ../src
    ];
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes."gds-uim-0.1.0" = "sha256-yVojxpvvejy2NBKgcKsffBMqrPqe6YOJ94vtjCUz7dQ=";
    outputHashes."bas-client-0.1.0" = "sha256-L+pNyI0blIoROxDEBjWW4waAtLBLCySGLqCYdP7yUw4=";
  };

  nativeBuildInputs = [ pkg-config cmake makeWrapper copyDesktopItems rustPlatform.bindgenHook ];
  nativeCheckInputs = [ pkgs.dbus ] ++ extraNativeCheckInputs;
  buildInputs = libraries;
  dontUseCmakeConfigure = true;
  cargoBuildFlags = [ "--bin" binary ];
  buildFeatures = features;
  checkFeatures = features;
  cargoTestFlags = [ "--bin" binary "--lib" ];

  desktopItems = [ (makeDesktopItem (desktopItem // {
    terminal = false;
    startupWMClass = desktopItem.name;
  })) ];

  inherit postInstall checkFlags;

  postFixup = ''
    wrapProgram "$out/bin/${binary}" \
      --prefix PATH : ${lib.makeBinPath [ pkgs.xdg-utils pkgs.xdg-terminal-exec ]} \
      --prefix LD_LIBRARY_PATH : "/run/opengl-driver/lib:${lib.makeLibraryPath libraries}"
  '';

  meta = {
    inherit description;
    homepage = "https://github.com/4t145/gds";
    mainProgram = binary;
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
  };
}
