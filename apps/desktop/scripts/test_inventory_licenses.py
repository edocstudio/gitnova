import json
import pathlib
import tempfile
import unittest
from importlib.util import module_from_spec, spec_from_file_location

spec = spec_from_file_location('inventory', pathlib.Path(__file__).with_name('inventory-licenses.py'))
module = module_from_spec(spec)
spec.loader.exec_module(module)


class SupplementalEvidence(unittest.TestCase):
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
