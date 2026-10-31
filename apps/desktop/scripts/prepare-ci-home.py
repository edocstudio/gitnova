#!/usr/bin/env python3
"""Give disposable hosted CI Git processes deterministic line endings, no identity.

Never writes the real home or Git configuration. Toolchain caches stay at their
original absolute paths when the next CI step adopts the new HOME.
"""
import os
import pathlib
import tempfile


def prepare(temporary, env_file, environment, original_home):
    root = pathlib.Path(tempfile.mkdtemp(prefix='gitnova-ci-home-', dir=temporary))
    (root / '.gitconfig').write_text('[core]\n\tautocrlf = false\n', encoding='utf-8')
    values = {'HOME': str(root), 'XDG_CONFIG_HOME': str(root / '.config'),
              'CARGO_HOME': environment.get('CARGO_HOME') or str(original_home / '.cargo'),
              'RUSTUP_HOME': environment.get('RUSTUP_HOME') or str(original_home / '.rustup')}
    if any('\n' in value or '\r' in value for value in values.values()):
        raise ValueError('Unexpected newline in CI path')
    with pathlib.Path(env_file).open('a', encoding='utf-8', newline='\n') as out:
        out.write(''.join(f'{key}={value}\n' for key, value in values.items()))
    return root


if __name__ == '__main__':
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise SystemExit('Only run inside a disposable GitHub Actions runner.')
    prepare(os.environ['RUNNER_TEMP'], os.environ['GITHUB_ENV'], os.environ, pathlib.Path.home())
    print('Prepared isolated CI Git home; real Git configuration and identity untouched.')
