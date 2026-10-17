import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import io
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('public_audit', ROOT / 'tools/release/audit_public.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class PublicReleaseTests(unittest.TestCase):
    def test_removed_private_file_is_still_rejected_in_history(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def git(*words):
                return subprocess.check_output(['git', *words], cwd=root, stderr=subprocess.DEVNULL, timeout=15)
            git('init')
            git('config', 'user.name', 'Release Test')
            git('config', 'user.email', 'test@example.invalid')
            (root / 'README.md').write_text('Public project', encoding='utf-8')
            git('add', '.')
            git('commit', '-m', 'Public root')
            private = root / 'experiments' / 'run' / 'result.json'
            private.parent.mkdir(parents=True)
            private.write_text('{}', encoding='utf-8')
            git('add', '.')
            git('commit', '-m', 'Private file')
            git('rm', 'experiments/run/result.json')
            git('commit', '-m', 'Remove file')
            with patch.object(audit, 'ROOT', root):
                report = audit.audit_revision('HEAD')
            self.assertEqual(report['commits'], 3)
            self.assertTrue(any('experiments/run/result.json' in error for error in report['errors']))

    def test_nested_binary_paths_are_rejected(self):
        content = io.BytesIO()
        machine = ('C' + ':/' + 'Users/' + 'private-user/' + 'source.rs').encode()
        with zipfile.ZipFile(content, 'w') as jar:
            jar.writestr('native/example.dll', machine)
        self.assertTrue(audit.inspect('app/example.jar', content.getvalue()))

    def test_public_fixture_paths_are_allowed_but_gui_inputs_are_rejected(self):
        self.assertEqual(audit.inspect('rust/fixtures/games/1P.txt', b'public-test'), [])
        self.assertTrue(audit.inspect('1P.txt', b'private-input'))

    def test_standard_jdk_file_uri_example_is_not_a_windows_user_path(self):
        self.assertEqual(audit.inspect('runtime-template.properties', b'# file:///users/auser/catalog/catalog.xml'), [])


if __name__ == '__main__':
    unittest.main()
