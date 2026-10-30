import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(sys.argv.pop(1)).resolve()

class CliReview(unittest.TestCase):
    def test_install_reinstall_remove_all_cross_device(self):
        with tempfile.TemporaryDirectory(dir='/tmp') as root, tempfile.TemporaryDirectory(dir='/dev/shm') as state:
            root = Path(root)
            script = root/'project/scripts/install-dev.py'
            script.parent.mkdir(parents=True)
            shutil.copy2(SOURCE, script)
            for app in ('files', 'agents'):
                binary = root/'project/target/debug'/f'gds-{app}'
                binary.parent.mkdir(parents=True, exist_ok=True)
                binary.touch()
            home = root/'home'
            data = root/'data'
            desktop = data/'applications'
            desktop.mkdir(parents=True)
            files_entry = desktop/'io.github.fourt145.GdsFiles.desktop'
            agents_entry = desktop/'io.github.fourt145.GdsAgents.desktop'
            files_entry.write_text('[Desktop Entry]\nName=Original\n')
            files_entry.chmod(0o600)
            agents_entry.symlink_to('relative-original.desktop')
            env = dict(os.environ, HOME=str(home), XDG_DATA_HOME=str(data), XDG_STATE_HOME=state)
            def run(*args):
                result = subprocess.run([sys.executable, str(script), *args], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
            run('all', '/nix/store/fake-environment')
            run('all', '/nix/store/fake-environment')
            for app in ('files', 'agents'):
                launcher = home/'.local/bin'/f'gds-{app}'
                self.assertTrue(os.access(launcher, os.X_OK))
                self.assertIn('Managed by GDS', launcher.read_text())
            run('--remove', 'all')
            self.assertEqual(files_entry.read_text(), '[Desktop Entry]\nName=Original\n')
            self.assertEqual(files_entry.stat().st_mode & 0o777, 0o600)
            self.assertTrue(agents_entry.is_symlink())
            self.assertEqual(os.readlink(agents_entry), 'relative-original.desktop')
            self.assertFalse((home/'.local/bin/gds-files').exists())
            self.assertFalse((home/'.local/bin/gds-agents').exists())
            run('--remove', 'all')
            self.assertEqual(list((Path(state)/'gds/development').iterdir()), [])

unittest.main(verbosity=2)
