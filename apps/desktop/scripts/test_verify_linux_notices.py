from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase, main

spec = spec_from_file_location('linux_notices', Path(__file__).with_name('verify-linux-notices.py'))
module = module_from_spec(spec)
spec.loader.exec_module(module)


class NativeNoticesTests(TestCase):
    def test_requires_referenced_text_inside_artifact(self):
        with TemporaryDirectory() as folder, TemporaryDirectory() as outside:
            root = Path(folder)
            notice = root / 'usr/share/doc/library/copyright'
            notice.parent.mkdir(parents=True)
            notice.write_text('See /usr/share/common-licenses/LGPL-2.1.\n', encoding='utf-8')
            with self.assertRaises(FileNotFoundError):
                module.inspect_notices(root)
            license_path = root / 'usr/share/common-licenses/LGPL-2.1'
            license_path.parent.mkdir(parents=True)
            license_path.write_text('Original upstream text\n', encoding='utf-8')
            report = module.inspect_notices(root)
            self.assertFalse(report['auditComplete'])
            self.assertEqual(report['referencedLicenseTexts'][0]['path'], 'usr/share/common-licenses/LGPL-2.1')
            license_path.unlink()
            external = Path(outside) / 'license'
            external.write_text('Outside text', encoding='utf-8')
            license_path.symlink_to(external)
            with self.assertRaisesRegex(ValueError, 'escapes'):
                module.inspect_notices(root)

    def test_does_not_certify_an_empty_collection(self):
        with TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, 'No bundled'):
                module.inspect_notices(Path(folder))


if __name__ == '__main__':
    main()
