#!/usr/bin/env python3
"""Install local launchers for Cargo's mutable debug binaries."""

import argparse
import errno
import json
import os
from pathlib import Path
import shlex
import shutil
import tempfile

MARKER = "# Managed by GDS install-dev"
APPLICATIONS = {
    "files": ("Files", "System;FileTools;FileManager;", "system-file-manager"),
    "agents": ("Agents", "Development;Utility;", "applications-development"),
}


def write(path, text, mode):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as temporary:
        temporary.write(text.encode())
        temporary.flush()
        os.fchmod(temporary.fileno(), mode)
        temporary_path = Path(temporary.name)
    try:
        temporary_path.replace(path)
    finally:
        temporary_path.unlink(missing_ok=True)


def save_original(path, backup):
    metadata = backup.with_suffix(".json")
    if metadata.exists():
        return
    backup.parent.mkdir(parents=True, exist_ok=True)
    exists = path.exists() or path.is_symlink()
    if exists:
        shutil.copy2(path, backup, follow_symlinks=False)
    write(metadata, json.dumps({"exists": exists}), 0o600)


def restore(path, backup):
    metadata = backup.with_suffix(".json")
    if not metadata.exists():
        return
    if path.is_symlink() or not path.is_file() or MARKER not in path.read_text():
        raise RuntimeError(f"development entry changed externally; keeping {path}")
    original = json.loads(metadata.read_text())
    if original["exists"]:
        try:
            backup.replace(path)
        except OSError as error:
            if error.errno != errno.EXDEV:
                raise
            # Stage on the destination filesystem, then publish atomically. Do
            # not consume the backup until copying AND replacement succeed.
            # copy2 without following links preserves relative/dangling links
            # as well as the regular-file metadata saved by save_original.
            with tempfile.TemporaryDirectory(dir=path.parent, prefix=".gds-restore-") as temporary:
                staged = Path(temporary) / "original"
                shutil.copy2(backup, staged, follow_symlinks=False)
                staged.replace(path)
            backup.unlink()
    else:
        path.unlink()
    metadata.unlink()


def desktop_argument(path):
    escaped = str(path).replace("\\", "\\\\").replace('"', '\\"')
    escaped = escaped.replace("`", "\\`").replace("$", "\\$").replace("%", "%%")
    return f'"{escaped}"'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--remove", action="store_true")
    parser.add_argument("application", choices=["all", *APPLICATIONS])
    parser.add_argument("environment", nargs="?")
    args = parser.parse_args()
    if not args.remove and not args.environment:
        parser.error("installation requires a recorded Nix development environment")

    project = Path(__file__).resolve().parent.parent
    state = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state") / "gds/development"
    applications = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share") / "applications"
    selected = APPLICATIONS if args.application == "all" else [args.application]
    for app in selected:
        title, categories, icon = APPLICATIONS[app]
        binary = project / "target/debug" / f"gds-{app}"
        launcher = Path.home() / ".local/bin" / f"gds-{app}"
        desktop = applications / f"io.github.fourt145.Gds{title}.desktop"
        backups = [(launcher, state / f"{app}-launcher.previous"), (desktop, state / f"{app}-desktop.previous")]
        if args.remove:
            for path, backup in backups:
                restore(path, backup)
            print(f"Restored previous {title} entries")
            continue

        if not binary.is_file():
            raise RuntimeError(f"build the development binary first: {binary}")
        if launcher.exists() or launcher.is_symlink():
            if launcher.is_symlink() or MARKER not in launcher.read_text():
                raise RuntimeError(f"refusing to replace an unrelated command: {launcher}")
        nix = shutil.which("nix")
        if not nix:
            raise RuntimeError("nix is required to launch the development environment")
        command = shlex.join([nix, "develop", str(Path(args.environment).absolute()), "--command", str(binary)])
        for path, backup in backups:
            save_original(path, backup)
        write(launcher, f'#!/usr/bin/env bash\n{MARKER}\nexec {command} "$@"\n', 0o755)
        arguments = " -- %f" if app == "files" else ""
        mime = "MimeType=inode/directory;\n" if app == "files" else ""
        write(desktop, f"""{MARKER}
[Desktop Entry]
Type=Application
Name=GDS {title} (Dev)
Exec={desktop_argument(launcher)}{arguments}
Icon={icon}
Categories={categories}
StartupWMClass=io.github.fourt145.Gds{title}
Terminal=false
{mime}""", 0o644)
        print(f"Installed {title} development launcher: {launcher} -> {binary}")


if __name__ == "__main__":
    main()
