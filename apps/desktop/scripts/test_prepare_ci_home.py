import pathlib
import tempfile
import unittest
from importlib.util import module_from_spec, spec_from_file_location

spec = spec_from_file_location('ci_home', pathlib.Path(__file__).with_name('prepare-ci-home.py'))
module = module_from_spec(spec)
spec.loader.exec_module(module)


class IsolatedHome(unittest.TestCase):
    def test_preserves_real_configuration_and_toolchain_paths_without_creating_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            original = root / 'original'
            original.mkdir()
            (original / '.gitconfig').write_bytes(b'private fixture bytes')
            env_file = root / 'env'
            fresh = module.prepare(root, env_file, {'CARGO_HOME': str(root / 'cargo')}, original)
            self.assertEqual((original / '.gitconfig').read_bytes(), b'private fixture bytes')
            self.assertNotIn('user', (fresh / '.gitconfig').read_text())
            self.assertIn('autocrlf = false', (fresh / '.gitconfig').read_text())
            values = dict(line.split('=', 1) for line in env_file.read_text().splitlines())
            self.assertEqual(values['HOME'], str(fresh))
            self.assertEqual(values['CARGO_HOME'], str(root / 'cargo'))
            self.assertEqual(values['RUSTUP_HOME'], str(original / '.rustup'))


if __name__ == '__main__':
    unittest.main()
