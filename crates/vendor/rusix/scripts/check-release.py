#!/usr/bin/env python3
"""Verify crate archives through independent consumers before publishing."""

import argparse
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def run(*args, cwd=ROOT, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-dirty', action='store_true')
    parser.add_argument('--skip-evaluation', action='store_true')
    args = parser.parse_args()
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    package = ['cargo', 'package', '--workspace', '--locked', '--offline']
    if args.allow_dirty:
        package.append('--allow-dirty')
    run(*package)

    with tempfile.TemporaryDirectory(prefix='rusix-release-') as directory:
        scratch = Path(directory)
        crates = {}
        for name in ['rusix', 'rusix-derive']:
            archive = ROOT / 'target' / 'package' / f'{name}-{version}.crate'
            with tarfile.open(archive) as source:
                source.extractall(scratch, filter='data')
            crate = scratch / f'{name}-{version}'
            crates[name] = crate
            assert (crate / 'LICENSE').is_file(), f'{name} is missing its licence'
            manifest = tomllib.loads((crate / 'Cargo.toml').read_text())
            assert not manifest.get('test'), 'repository integration tests leaked into archive'
            assert not manifest.get('example'), 'repository examples leaked into archive'
            if name == 'rusix-derive':
                assert not manifest.get('bin'), 'spacing checker leaked into macro archive'
            else:
                assert (crate / 'README.md').read_text() == (ROOT / 'README.md').read_text(), \
                    'main crate README differs from the shared repository README'
                assert (crate / 'LICENSE-NIXPKGS').is_file()
                assert manifest['dependencies']['rusix-derive']['version'] == f'={version}'
                assert 'path' not in manifest['dependencies']['rusix-derive']

        fixture = (ROOT / 'crates/rusix/tests/consumers/library.rs.txt').read_text()
        fixture = fixture.replace('#[test]', '#[cfg(feature = "evaluation")]\n#[test]')
        for dependency in ['rusix', 'renamed_rusix']:
            consumer = scratch / dependency
            (consumer / 'src').mkdir(parents=True)
            (consumer / 'Cargo.toml').write_text(f'''[package]
name = "release-consumer"
version = "0.0.0"
edition = "2024"

[workspace]

[features]
evaluation = ["{dependency}/evaluation"]

[dependencies]
{dependency} = {{ package = "rusix", version = "={version}", default-features = false }}

[patch.crates-io]
rusix = {{ path = "{crates['rusix'].as_posix()}" }}
rusix-derive = {{ path = "{crates['rusix-derive'].as_posix()}" }}
''')
            (consumer / 'src/lib.rs').write_text(fixture.replace('rusix::', f'{dependency}::'))
            (consumer / 'src/main.rs').write_text('''fn main() {
    let generated = release_consumer::artifact().expect("valid Rust configuration");
    assert!(!generated.source.is_empty());
    assert!(!generated.spans.is_empty());
    println!("compiled without Nix");
}
''')
            env = os.environ.copy()
            env['CARGO_TARGET_DIR'] = str(ROOT / 'target' / 'release-consumers')
            run('cargo', 'generate-lockfile', '--offline', cwd=consumer, env=env)
            run('cargo', 'build', '--locked', '--offline', cwd=consumer, env=env)
            no_nix = env.copy()
            no_nix['PATH'] = str(scratch / 'empty-path')
            executable = Path(env['CARGO_TARGET_DIR']) / 'debug' / 'release-consumer'
            run(str(executable), cwd=consumer, env=no_nix)
            if not args.skip_evaluation:
                env['RUSIX_TEST_NIXPKGS'] = str(ROOT / 'vendor' / 'nixpkgs')
                run('cargo', 'test', '--features', 'evaluation', '--locked', '--offline',
                    cwd=consumer, env=env)
    print('Release archives and ordinary/renamed consumers verified.')


if __name__ == '__main__':
    main()
