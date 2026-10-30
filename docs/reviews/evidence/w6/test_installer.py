import pathlib
import tempfile
import unittest
import install_dev as m

class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = pathlib.Path(self.temp.name)
        self.path = root / 'data' / 'app.desktop'
        self.path.parent.mkdir()
        self.backup = root / 'state' / 'app.previous'

    def test_restore_existing(self):
        self.path.write_text('original')
        m.save_original(self.path, self.backup)
        m.write(self.path, m.MARKER, 0o644)
        m.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), 'original')
        self.assertFalse(self.backup.with_suffix('.json').exists())

    def test_restore_absent(self):
        m.save_original(self.path, self.backup)
        m.write(self.path, m.MARKER, 0o644)
        m.restore(self.path, self.backup)
        self.assertFalse(self.path.exists())

    def test_preserve_external_edit(self):
        self.path.write_text('original')
        m.save_original(self.path, self.backup)
        self.path.write_text('external')
        with self.assertRaises(RuntimeError):
            m.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), 'external')
        self.assertEqual(self.backup.read_text(), 'original')

    def test_preserve_external_symlink(self):
        self.path.write_text('original')
        m.save_original(self.path, self.backup)
        self.path.unlink()
        target = self.path.parent / 'external'
        target.write_text(m.MARKER)
        self.path.symlink_to(target)
        with self.assertRaises(RuntimeError):
            m.restore(self.path, self.backup)
        self.assertTrue(self.path.is_symlink())

    def test_backup_idempotent(self):
        self.path.write_text('original')
        m.save_original(self.path, self.backup)
        m.write(self.path, m.MARKER, 0o644)
        m.save_original(self.path, self.backup)
        self.assertEqual(self.backup.read_text(), 'original')

    def test_restore_without_metadata_noop(self):
        self.path.write_text('external')
        m.restore(self.path, self.backup)
        self.assertEqual(self.path.read_text(), 'external')

if __name__ == '__main__':
    unittest.main()
