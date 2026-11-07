import json
import pathlib
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from unittest.mock import patch
from importlib.util import module_from_spec, spec_from_file_location
from os import devnull, environ
from subprocess import run

spec = spec_from_file_location('inventory', pathlib.Path(__file__).with_name('inventory-licenses.py'))
module = module_from_spec(spec)
spec.loader.exec_module(module)


class SupplementalEvidence(unittest.TestCase):
    def test_windows_checkout_preserves_pinned_license_bytes(self):
        project = pathlib.Path(__file__).resolve().parents[3]
        entries = json.loads((project / 'docs/licenses/supplemental-evidence.json').read_text())['components']
        env = {key: value for key, value in environ.items() if not key.startswith('GIT_')}
        env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=devnull)
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            def git(*args):
                run(['git', *args], cwd=root, env=env, check=True, capture_output=True, timeout=30)
            git('init', '-b', 'main')
            git('config', 'core.autocrlf', 'true')
            git('config', 'core.safecrlf', 'false')
            git('config', 'core.attributesFile', devnull)
            attributes = project / '.gitattributes'
            if attributes.exists():
                (root / '.gitattributes').write_bytes(attributes.read_bytes())
            files = {entry['file']: entry['sha256'] for entry in entries.values()}
            for name in files:
                target = root / 'docs/licenses' / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((project / 'docs/licenses' / name).read_bytes())
            git('add', '.')
            for name in files:
                (root / 'docs/licenses' / name).unlink()
            git('checkout-index', '--all', '--force')
            for name, expected in files.items():
                self.assertEqual(module.digest((root / 'docs/licenses' / name).read_bytes()), expected, name)

    def test_npm_utf8_metadata_under_windows_legacy_encoding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            desktop = root / 'apps/desktop'
            package = desktop / 'node_modules/fixture'
            package.mkdir(parents=True)
            (root / 'Cargo.lock').write_bytes(b'fixture lock')
            (root / 'docs/licenses').mkdir(parents=True)
            (root / 'docs/licenses/supplemental-evidence.json').write_text(
                json.dumps({'schemaVersion': 1, 'components': {}}), encoding='utf-8')
            (desktop / 'package-lock.json').write_text(json.dumps({'packages': {
                'node_modules/fixture': {'version': '1.0.0', 'license': 'MIT'}
            }}), encoding='utf-8')
            (package / 'package.json').write_text(json.dumps({
                'name': 'fixture', 'description': 'ก metadata', 'license': 'MIT'
            }, ensure_ascii=False), encoding='utf-8')
            license_bytes = 'MIT fixture ก notice'.encode('utf-8')
            (package / 'LICENSE').write_bytes(license_bytes)
            output = root / 'evidence'
            original_read = pathlib.Path.read_text
            original_write = pathlib.Path.write_text

            def windows_read(path, *args, **kwargs):
                kwargs.setdefault('encoding', 'cp1252')
                return original_read(path, *args, **kwargs)

            def windows_write(path, data, *args, **kwargs):
                kwargs.setdefault('encoding', 'cp1252')
                return original_write(path, data, *args, **kwargs)

            cargo = json.dumps({'packages': [], 'workspace_members': []}).encode('utf-8')
            with patch.object(module, 'ROOT', root), \
                 patch.object(module.subprocess, 'check_output', return_value=cargo), \
                 patch('sys.argv', ['inventory', '--target', 'x86_64-pc-windows-msvc', '--output', str(output)]), \
                 patch.object(pathlib.Path, 'read_text', windows_read), \
                 patch.object(pathlib.Path, 'write_text', windows_write), \
                 redirect_stdout(StringIO()):
                module.main()
            report = json.loads((output / 'dependency-inventory.json').read_text(encoding='utf-8'))
            self.assertEqual(report['target'], 'x86_64-pc-windows-msvc')
            self.assertEqual(report['components'][0]['id'], 'npm:fixture@1.0.0')
            self.assertIn(license_bytes, (output / 'third-party-license-evidence.txt').read_bytes())
            self.assertFalse(report['auditComplete'])

    def test_rejects_changed_text_revision_and_path_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            folder = root / 'crate'
            folder.mkdir()
            (folder / '.cargo_vcs_info.json').write_text(json.dumps({'git': {'sha1': 'published-revision'}}))
            upstream = root / 'docs/licenses/upstream'
            upstream.mkdir(parents=True)
            text = b'original fixture license'
            (upstream / 'LICENSE').write_bytes(text)
            extra = {'file': 'upstream/LICENSE', 'license': 'MIT', 'revision': 'published-revision', 'sha256': module.digest(text)}
            self.assertEqual(module.supplemental_text(root, folder, 'cargo', 'MIT', extra)[1], text)
            for change in ({'revision': 'other'}, {'file': '../../../outside'}, {'sha256': 'wrong'}):
                with self.assertRaises(ValueError):
                    module.supplemental_text(root, folder, 'cargo', 'MIT', {**extra, **change})
            with self.assertRaises(ValueError):
                module.supplemental_text(root, folder, 'cargo', 'BSD-3-Clause', extra)


if __name__ == '__main__':
    unittest.main()
