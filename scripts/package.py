"""Package built release and clean source tree using explicit allowlists (stdlib only).
Run after cargo build --release --locked. Never includes logs, frames or local paths.
Dependency license texts come from exact downloaded Cargo packages, not guessed copies.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / 'dist'


def package():
    exe = ROOT / 'target/release/capture-view.exe'
    if not exe.is_file():
        raise SystemExit('Build first: cargo build --release --locked')
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline'], cwd=ROOT))
    DIST.mkdir(exist_ok=True)
    documents = ['README.md', 'LICENSE', 'THIRD_PARTY.md', 'CONTRIBUTING.md',
                 'docs/README.fr.md', 'docs/ARCHITECTURE.md', 'docs/TESTING.md']
    binary_zip = DIST / 'CaptureView-windows-x64.zip'
    with zipfile.ZipFile(binary_zip, 'w', zipfile.ZIP_DEFLATED) as z:
        z.write(exe, 'CaptureView.exe')
        for name in documents:
            z.write(ROOT / name, name)
        for pkg in metadata['packages']:
            if pkg['name'] == 'capture-view':
                continue
            folder = Path(pkg['manifest_path']).parent
            notices = [p for p in folder.iterdir() if p.is_file() and
                       p.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE'))]
            if not notices:
                raise RuntimeError(f"No license text found for {pkg['name']}")
            for path in notices:
                z.write(path, f"third-party-licenses/{pkg['name']}-{pkg['version']}/{path.name}")
    source_zip = DIST / 'CaptureView-source.zip'
    names = documents + ['Cargo.toml', 'Cargo.lock', 'build.rs', 'app.manifest',
                         '.cargo/config.toml', '.gitignore', '.github/workflows/ci.yml',
                         'integration_test.py', 'scripts/package.py', 'scripts/ui_regression.py']
    names += [p.relative_to(ROOT).as_posix() for p in sorted((ROOT / 'src').glob('*.rs'))]
    with zipfile.ZipFile(source_zip, 'w', zipfile.ZIP_DEFLATED) as z:
        for name in names:
            z.write(ROOT / name, f'CaptureView/{name}')
    lines = []
    for path in [binary_zip, source_zip]:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f'{digest}  {path.name}')
        print(path.name, path.stat().st_size, 'bytes;', digest)
    (DIST / 'SHA256SUMS.txt').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    # Read back the actual archives, validate CRC and required entries.
    for path in [binary_zip, source_zip]:
        with zipfile.ZipFile(path) as z:
            assert z.testzip() is None, f'Invalid ZIP: {path}'
            assert len(z.namelist()) == len(set(z.namelist())), 'Duplicate member'
            assert not any(n.endswith(('.log', '.bmp')) for n in z.namelist())
    print('Archive CRC, unique entries and media/log exclusion verified.')


if __name__ == '__main__':
    package()
