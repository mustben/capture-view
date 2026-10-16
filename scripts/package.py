"""Build locked release sources, validate x64 GUI PE, then stage distribution files.
Only explicit source/document allowlists are distributed. No logs or private frames.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / 'dist'


def validate_pe(data):
    def read(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(data):
            raise ValueError('Truncated PE header')
        return struct.unpack_from(fmt, data, offset)[0]
    if data[:2] != b'MZ':
        raise ValueError('Missing MZ executable signature')
    pe = read('<I', 0x3c)
    if pe < 64 or data[pe:pe+4] != b'PE\0\0':
        raise ValueError('Missing or out-of-bounds PE signature')
    if read('<H', pe+4) != 0x8664:
        raise ValueError('Expected AMD64 executable')
    sections = read('<H', pe+6)
    optional_size = read('<H', pe+20)
    optional = pe+24
    if optional_size < 112 or optional+optional_size > len(data):
        raise ValueError('Truncated PE32+ optional header')
    if read('<H', optional) != 0x20b or read('<H', optional+68) != 2:
        raise ValueError('Expected PE32+ Windows GUI subsystem')
    table = optional+optional_size
    if not sections or table+40*sections > len(data):
        raise ValueError('Invalid PE section table')
    for i in range(sections):
        entry = table+40*i
        size, offset = read('<I', entry+16), read('<I', entry+20)
        if size and (offset < table+40*sections or offset+size > len(data)):
            raise ValueError('Truncated PE section data')


def executable_path(metadata, build_output):
    """Use Cargo's actual artifact, including config/env/explicit target and custom directories."""
    artifacts = [json.loads(line) for line in build_output.splitlines() if line.strip()]
    paths = [Path(item['executable']) for item in artifacts
             if item.get('reason') == 'compiler-artifact'
             and item.get('target', {}).get('name') == 'capture-view'
             and 'bin' in item.get('target', {}).get('kind', [])
             and item.get('executable')]
    if len(paths) != 1:
        raise ValueError('Cargo did not report exactly one capture-view executable')
    exe = paths[0].resolve()
    if not exe.is_relative_to(Path(metadata['target_directory']).resolve()):
        raise ValueError('Executable is outside Cargo target_directory')
    return exe


def validate_zip(path, required):
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        if z.testzip() is not None or len(names) != len(set(names)):
            raise ValueError('ZIP CRC failure or duplicate entries')
        if not set(required) <= set(names):
            raise ValueError('ZIP missing required entries')
        if any(n.endswith(('.log', '.bmp', '.png', '.wav', '.mp4')) for n in names):
            raise ValueError('Private diagnostic/media file in ZIP')


def publish(staged, destination, names):
    """All outputs validated before replacement; rollback on ordinary I/O failures.

    Multiple file replacements are not crash-atomic as a group. SHA256SUMS is last.
    """
    backups = staged / 'previous'
    backups.mkdir()
    changed = []
    try:
        for name in names:
            final = destination/name
            if final.exists():
                shutil.copy2(final, backups/name)
            changed.append(name)
            (staged/name).replace(final)
    except BaseException:
        for name in reversed(changed):
            if (backups/name).exists():
                (backups/name).replace(destination/name)
            else:
                (destination/name).unlink(missing_ok=True)
        raise


def package(target=None):
    # Integrated build: PE structure alone does not establish source provenance.
    command = ['cargo', 'build', '--release', '--locked', '--message-format=json-render-diagnostics']
    if target:
        command += ['--target', target]
    build = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline'], cwd=ROOT))
    exe = executable_path(metadata, build.stdout)
    validate_pe(exe.read_bytes())
    DIST.mkdir(exist_ok=True)
    documents = ['README.md', 'LICENSE', 'THIRD_PARTY.md', 'CONTRIBUTING.md',
                 'docs/README.fr.md', 'docs/ARCHITECTURE.md', 'docs/TESTING.md',
                 'docs/REVIEW_CORRECTIONS.md']
    names = documents + ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'build.rs', 'app.manifest',
                         '.cargo/config.toml', '.gitignore', '.github/workflows/ci.yml',
                         'integration_test.py', 'scripts/package.py', 'scripts/test_package.py',
                         'scripts/ui_regression.py', 'scripts/test_opening.py']
    names += [p.relative_to(ROOT).as_posix() for p in sorted((ROOT/'src').glob('*.rs'))]
    with tempfile.TemporaryDirectory(prefix='.package-', dir=DIST) as temp:
        staged = Path(temp)
        binary = staged/'CaptureView-windows-x64.zip'
        required = ['CaptureView.exe'] + documents
        with zipfile.ZipFile(binary, 'w', zipfile.ZIP_DEFLATED) as z:
            z.write(exe, 'CaptureView.exe')
            for name in documents:
                z.write(ROOT/name, name)
            for pkg in metadata['packages']:
                if pkg['name'] == 'capture-view':
                    continue
                folder = Path(pkg['manifest_path']).parent
                notices = sorted(p for p in folder.iterdir() if p.is_file() and
                                 p.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE')))
                if not notices:
                    raise RuntimeError(f"No license text found for {pkg['name']}")
                for path in notices:
                    member = f"third-party-licenses/{pkg['name']}-{pkg['version']}/{path.name}"
                    required.append(member)
                    z.write(path, member)
        source = staged/'CaptureView-source.zip'
        with zipfile.ZipFile(source, 'w', zipfile.ZIP_DEFLATED) as z:
            for name in names:
                z.write(ROOT/name, f'CaptureView/{name}')
        validate_zip(binary, required)
        validate_zip(source, [f'CaptureView/{n}' for n in names])
        lines = [f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}' for p in [binary, source]]
        sums = staged/'SHA256SUMS.txt'
        sums.write_text('\n'.join(lines)+'\n', encoding='utf-8')
        for line in sums.read_text().splitlines():
            digest, name = line.split('  ')
            if hashlib.sha256((staged/name).read_bytes()).hexdigest() != digest:
                raise ValueError('SHA-256 mismatch')
        publish(staged, DIST, [binary.name, source.name, sums.name])
    print('Built release; PE, ZIP entries, CRC, licenses and SHA-256 verified.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', help='Explicit Cargo target triple (must produce x64 Windows GUI)')
    package(parser.parse_args().target)
