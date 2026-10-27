{ lib, pkgs, gds-files }:
let
  python = pkgs.python3.withPackages (ps: [ ps.dbus-next ]);
in
pkgs.stdenvNoCC.mkDerivation {
  pname = "xdg-desktop-portal-gds";
  version = gds-files.version;
  src = ../../../src/bin/files/portal.py;
  dontUnpack = true;
  nativeBuildInputs = [ pkgs.makeWrapper ];
  installPhase = ''
    install -Dm644 "$src" "$out/libexec/gds-files-portal.py"
    makeWrapper ${python}/bin/python "$out/libexec/xdg-desktop-portal-gds" \
      --add-flags "$out/libexec/gds-files-portal.py ${gds-files}/bin/gds-files"
    mkdir -p "$out/share/xdg-desktop-portal/portals" "$out/share/dbus-1/services"
    cat > "$out/share/xdg-desktop-portal/portals/gds.portal" <<EOF
    [portal]
    DBusName=org.freedesktop.impl.portal.desktop.gds
    Interfaces=org.freedesktop.impl.portal.FileChooser;
    EOF
    cat > "$out/share/dbus-1/services/org.freedesktop.impl.portal.desktop.gds.service" <<EOF
    [D-BUS Service]
    Name=org.freedesktop.impl.portal.desktop.gds
    Exec=$out/libexec/xdg-desktop-portal-gds
    SystemdService=xdg-desktop-portal-gds.service
    EOF
    mkdir -p "$out/lib/systemd/user"
    cat > "$out/lib/systemd/user/xdg-desktop-portal-gds.service" <<EOF
    [Unit]
    Description=GDS Files FileChooser portal
    PartOf=graphical-session.target
    After=graphical-session.target

    [Service]
    Type=dbus
    BusName=org.freedesktop.impl.portal.desktop.gds
    ExecStart=$out/libexec/xdg-desktop-portal-gds
    Slice=session.slice
    EOF
  '';
  meta = {
    description = "FileChooser portal using GDS Files with GTK fallback";
    platforms = lib.platforms.linux;
  };
}
