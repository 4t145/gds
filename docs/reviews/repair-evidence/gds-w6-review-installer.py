import importlib.util
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('install_dev', sys.argv.pop(1))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

class Review(unittest.TestCase):
    def setUp(self):
        self.data_tmp = tempfile.TemporaryDirectory(dir='/tmp')
        self.state_tmp = tempfile.TemporaryDirectory(dir='/dev/shm')
        self.addCleanup(self.data_tmp.cleanup)
        self.addCleanup(self.state_tmp.cleanup)
        self.path = Path(self.data_tmp.name) / 'entry'
        self.backup = Path(self.state_tmp.name) / 'entry.previous'
        assert self.path.parent.stat().st_dev != self.backup.parent.stat().st_dev

    def installed(self):
        m.save_original(self.path, self.backup)
        m.write(self.path, m.MARKER, 0o755)

    def test_cross_device_regular_metadata(self):
        self.path.write_bytes(b'original\x00content')
        self.path.chmod(0o640)
        os.utime(self.path, ns=(1700000000000000000, 1700000000123456789))
        before = self.path.stat()
        self.installed()
        m.restore(self.path, self.backup)
        self.assertEqual(self.path.read_bytes(), b'original\x00content')
        self.assertEqual(stat.S_IMODE(self.path.stat().st_mode), 0o640)
        self.assertEqual(self.path.stat().st_mtime_ns, before.st_mtime_ns)
        self.assertFalse(self.backup.exists())
        self.assertFalse(self.backup.with_suffix('.json').exists())

    def test_cross_device_dangling_relative_symlink(self):
        self.path.symlink_to('../not-present')
        self.installed()
        m.restore(self.path, self.backup)
        self.assertTrue(self.path.is_symlink())
        self.assertEqual(os.readlink(self.path), '../not-present')
        self.assertFalse(self.backup.is_symlink())

    def test_failed_publication_preserves_backup(self):
        self.path.write_text('original')
        self.installed()
        real_replace = Path.replace
        def reject_entry(source, target):
            if Path(target) == self.path:
                raise PermissionError('injected publication failure')
            return real_replace(source, target)
        with patch.object(Path, 'replace', reject_entry):
            with self.assertRaises(OSError):
                m.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), m.MARKER)
        self.assertEqual(self.backup.read_text(), 'original')
        self.assertTrue(self.backup.with_suffix('.json').exists())
        self.assertEqual(sorted(p.name for p in self.path.parent.iterdir()), ['entry'])

if __name__ == '__main__':
    unittest.main(verbosity=2)
