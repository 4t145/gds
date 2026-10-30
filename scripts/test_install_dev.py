"""Installer restore regressions; run with python3 -m unittest discover -s scripts."""
import errno
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("install_dev", Path(__file__).with_name("install-dev.py"))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class RestoreTests(unittest.TestCase):
    def setUp(self):
        self.local = tempfile.TemporaryDirectory()
        self.addCleanup(self.local.cleanup)
        self.path = Path(self.local.name) / "app.desktop"
        self.backup = Path(self.local.name) / "state" / "app.previous"

    def prepare(self):
        installer.save_original(self.path, self.backup)
        installer.write(self.path, installer.MARKER, 0o644)

    def cross_device(self):
        try:
            remote = tempfile.TemporaryDirectory(dir="/dev/shm", prefix="gds-restore-test-")
        except OSError as error:
            self.skipTest(f"second writable filesystem unavailable: {error}")
        self.addCleanup(remote.cleanup)
        if os.stat(remote.name).st_dev == os.stat(self.local.name).st_dev:
            self.skipTest("/dev/shm is not a distinct filesystem")
        self.backup = Path(remote.name) / "app.previous"

    def assert_consumed(self):
        self.assertFalse(self.backup.exists() or self.backup.is_symlink())
        self.assertFalse(self.backup.with_suffix(".json").exists())

    def test_restore_existing_same_device(self):
        self.path.write_text("original")
        self.prepare()
        installer.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), "original")
        self.assert_consumed()

    def test_restore_absent(self):
        self.prepare()
        installer.restore(self.path, self.backup)
        self.assertFalse(self.path.exists())
        self.assert_consumed()

    def test_restore_cross_device_preserves_file_metadata(self):
        self.cross_device()
        self.path.write_text("original")
        self.path.chmod(0o751)
        os.utime(self.path, ns=(1600000000000000000, 1600000000123456789))
        self.prepare()
        expected = self.backup.stat()
        installer.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), "original")
        self.assertEqual(self.path.stat().st_mode, expected.st_mode)
        self.assertEqual(self.path.stat().st_mtime_ns, expected.st_mtime_ns)
        self.assert_consumed()

    def test_restore_cross_device_preserves_dangling_symlink(self):
        self.cross_device()
        self.path.symlink_to("missing-relative-target")
        self.prepare()
        expected = self.backup.lstat().st_mtime_ns
        installer.restore(self.path, self.backup)
        self.assertTrue(self.path.is_symlink())
        self.assertEqual(os.readlink(self.path), "missing-relative-target")
        self.assertEqual(self.path.lstat().st_mtime_ns, expected)
        self.assert_consumed()

    def test_failed_cross_device_copy_retains_backup_and_entry(self):
        self.cross_device()
        self.path.write_text("original")
        self.prepare()
        with mock.patch.object(installer.shutil, "copy2", side_effect=OSError(errno.ENOSPC, "injected full disk")):
            with self.assertRaises(OSError) as raised:
                installer.restore(self.path, self.backup)
        self.assertEqual(raised.exception.errno, errno.ENOSPC)
        self.assertEqual(self.path.read_text(), installer.MARKER)
        self.assertEqual(self.backup.read_text(), "original")
        self.assertTrue(self.backup.with_suffix(".json").exists())
        self.assertEqual(list(Path(self.local.name).iterdir()), [self.path])

    def test_failed_cross_device_publication_retains_backup_and_entry(self):
        self.cross_device()
        self.path.write_text("original")
        self.prepare()
        replace = Path.replace

        def fail_publication(source, target):
            if source != self.backup:
                raise OSError(errno.EACCES, "injected publication failure")
            return replace(source, target)

        with mock.patch.object(Path, "replace", fail_publication):
            with self.assertRaises(OSError) as raised:
                installer.restore(self.path, self.backup)
        self.assertEqual(raised.exception.errno, errno.EACCES)
        self.assertEqual(self.path.read_text(), installer.MARKER)
        self.assertEqual(self.backup.read_text(), "original")
        self.assertTrue(self.backup.with_suffix(".json").exists())
        self.assertEqual(list(Path(self.local.name).iterdir()), [self.path])

    def test_non_exdev_error_is_not_retried_as_copy(self):
        self.path.write_text("original")
        self.prepare()
        with mock.patch.object(Path, "replace", side_effect=OSError(errno.EACCES, "denied")):
            with mock.patch.object(installer.shutil, "copy2") as copy:
                with self.assertRaises(OSError):
                    installer.restore(self.path, self.backup)
                copy.assert_not_called()
        self.assertEqual(self.backup.read_text(), "original")
        self.assertTrue(self.backup.with_suffix(".json").exists())

    def test_external_edit_preserves_backup(self):
        self.path.write_text("original")
        self.prepare()
        self.path.write_text("external")
        with self.assertRaises(RuntimeError):
            installer.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), "external")
        self.assertEqual(self.backup.read_text(), "original")


if __name__ == "__main__":
    unittest.main()
