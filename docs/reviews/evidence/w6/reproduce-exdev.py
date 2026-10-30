"""Run from repository root with Nix shell Python; modifies only temporary dirs."""
import importlib.util
import pathlib
import sys
import tempfile

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("install_dev", "scripts/install-dev.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
with tempfile.TemporaryDirectory(prefix="gds-w6-") as data, tempfile.TemporaryDirectory(prefix="gds-w6-", dir="/dev/shm") as state:
    path = pathlib.Path(data) / "entry.desktop"
    backup = pathlib.Path(state) / "files-desktop.previous"
    path.write_text("ORIGINAL DESKTOP\n")
    module.save_original(path, backup)
    module.write(path, module.MARKER + "\nMANAGED DESKTOP\n", 0o644)
    assert path.stat().st_dev != backup.stat().st_dev, "requires separate filesystems"
    try:
        module.restore(path, backup)
    except OSError as error:
        print("restore_errno", error.errno)
        assert error.errno == 18
    else:
        raise AssertionError("expected EXDEV on baseline")
    print("original_restored", path.read_text() == "ORIGINAL DESKTOP\n")
    print("backup_retained", backup.exists())
