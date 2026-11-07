#!/usr/bin/env python3
"""Verify collected AppImage notices, not license compliance or source obligations."""
from argparse import ArgumentParser
from hashlib import sha256
from json import dumps
from pathlib import Path
from re import findall


def inspect_notices(root):
    root = root.resolve(strict=True)
    notices = []
    references = set()
    for path in sorted((root / 'usr/share/doc').glob('*/copyright')):
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(root) or not resolved.is_file():
            raise ValueError('Packaged copyright notice escapes the AppImage')
        if resolved.stat().st_size > 1024 * 1024:
            raise ValueError('Packaged copyright notice exceeds review limit')
        data = resolved.read_bytes()
        references.update(ref.rstrip('.') for ref in findall(
            r'/usr/share/common-licenses/[A-Za-z0-9.+-]+', data.decode('utf-8')))
        notices.append({'path': path.relative_to(root).as_posix(), 'sha256': sha256(data).hexdigest()})
    if not notices:
        raise ValueError('No bundled native library copyright notices found')
    texts = []
    for reference in sorted(references):
        path = root / reference.lstrip('/')
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(root) or not resolved.is_file():
            raise ValueError('Referenced license text escapes the AppImage')
        if not 0 < resolved.stat().st_size <= 1024 * 1024:
            raise ValueError('Referenced license text is empty or exceeds review limit')
        data = resolved.read_bytes()
        texts.append({'path': path.relative_to(root).as_posix(), 'sha256': sha256(data).hexdigest()})
    return {'schemaVersion': 1, 'kind': 'gitnova-appimage-native-notices',
            'auditComplete': False, 'copyrightNotices': notices, 'referencedLicenseTexts': texts,
            'limitations': ['File inclusion is not a shipped-library license audit.',
                            'License choices, corresponding source, relinking and SDK obligations require separate review.']}


def main():
    parser = ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = inspect_notices(args.root)
    digest = sha256()
    with args.artifact.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    report.update(artifact=args.artifact.name, artifactSha256=digest.hexdigest())
    args.output.write_text(dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f"Verified {len(report['copyrightNotices'])} native notices and {len(report['referencedLicenseTexts'])} referenced texts; audit remains incomplete.")


if __name__ == '__main__':
    main()
