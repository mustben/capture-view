"""No Windows or capture hardware required: PE fixtures and packaging fault injection."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import package as pkg


def pe(machine=0x8664, subsystem=2):
    data = bytearray(512)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 64)
    data[64:68] = b'PE\0\0'
    struct.pack_into('<H', data, 68, machine)
    struct.pack_into('<H', data, 70, 1)
    struct.pack_into('<H', data, 84, 112)
    struct.pack_into('<H', data, 88, 0x20b)
    struct.pack_into('<H', data, 156, subsystem)
    struct.pack_into('<II', data, 216, 272, 240)
    return data


class PEValidation(unittest.TestCase):
    def test_accept_x64_gui(self):
        pkg.validate_pe(pe())

    def test_reject_text_truncated_architecture_subsystem(self):
        for data in [b'not an executable', pe()[:100], pe(0x14c), pe(subsystem=3), pe()[:400]]:
            with self.subTest(size=len(data)), self.assertRaises(ValueError):
                pkg.validate_pe(data)
        data = pe()
        struct.pack_into('<I', data, 0x3c, 0xffffffff)
        with self.assertRaises(ValueError):
            pkg.validate_pe(data)

    def test_cargo_paths(self):
        metadata = {'target_directory': '/custom/cargo'}
        for path in ['/custom/cargo/release/capture-view.exe',
                     '/custom/cargo/x86_64-pc-windows-msvc/release/capture-view.exe']:
            output = json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'capture-view', 'kind': ['bin']}, 'executable': path})
            self.assertEqual(pkg.executable_path(metadata, output), Path(path))
        with self.assertRaises(ValueError):
            pkg.executable_path(metadata, '')


class Packaging(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.dist = self.root/'dist'
        self.dist.mkdir()
        self.target = self.root/'custom-target'
        exe = self.target/'release/capture-view.exe'
        exe.parent.mkdir(parents=True)
        exe.write_bytes(pe())
        # Populate the actual script allowlist from the repository, without local outputs.
        repository = Path(__file__).resolve().parents[1]
        for path in repository.rglob('*'):
            if path.is_file() and (path.suffix in ('.rs', '.py', '.toml', '.md', '.lock', '.yml', '.manifest') or path.name in ('LICENSE', '.gitignore')):
                if any(part in ('.git', 'target', 'dist') for part in path.relative_to(repository).parts):
                    continue
                copy = self.root/path.relative_to(repository)
                copy.parent.mkdir(parents=True, exist_ok=True)
                copy.write_bytes(path.read_bytes())
        dep = self.root/'dependency'
        dep.mkdir()
        (dep/'LICENSE').write_text('test license')
        self.metadata = {'target_directory': str(self.target), 'packages': [
            {'name': 'capture-view'}, {'name': 'fixture', 'version': '1', 'manifest_path': str(dep/'Cargo.toml')}]}

    def package(self):
        with patch.object(pkg, 'ROOT', self.root), patch.object(pkg, 'DIST', self.dist), \
             patch.object(pkg.subprocess, 'run') as build, \
             patch.object(pkg.subprocess, 'check_output', return_value=json.dumps(self.metadata).encode()):
            build.return_value.stdout = json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'capture-view', 'kind': ['bin']}, 'executable': str(self.target/'release/capture-view.exe')})
            pkg.package()
            build.assert_called_once_with(['cargo', 'build', '--release', '--locked', '--message-format=json-render-diagnostics'], cwd=self.root, check=True, stdout=pkg.subprocess.PIPE, text=True)

    def test_contents_hashes_and_licenses(self):
        self.package()
        with zipfile.ZipFile(self.dist/'CaptureView-windows-x64.zip') as z:
            self.assertIn('third-party-licenses/fixture-1/LICENSE', z.namelist())
            self.assertIsNone(z.testzip())
            self.assertEqual(len(z.namelist()), len(set(z.namelist())))
            pkg.validate_pe(z.read('CaptureView.exe'))
        with zipfile.ZipFile(self.dist/'CaptureView-source.zip') as z:
            self.assertIn('CaptureView/rust-toolchain.toml', z.namelist())
        for line in (self.dist/'SHA256SUMS.txt').read_text().splitlines():
            digest, name = line.split('  ')
            self.assertEqual(digest, hashlib.sha256((self.dist/name).read_bytes()).hexdigest())

    def test_mid_package_failure_preserves_previous_and_cleans_temps(self):
        self.package()
        old = {p.name: p.read_bytes() for p in self.dist.iterdir()}
        (self.root/'dependency/LICENSE').unlink()
        with self.assertRaises(RuntimeError):
            self.package()
        self.assertEqual(old, {p.name: p.read_bytes() for p in self.dist.iterdir()})

    def test_publish_failure_rolls_back(self):
        staged = self.root/'stage'
        staged.mkdir()
        for name in ['a', 'b']:
            (staged/name).write_text('new')
            (self.dist/name).write_text('old')
        original = Path.replace
        def fail_second(path, target):
            if path == staged/'b':
                raise OSError('injected replacement failure')
            return original(path, target)
        with patch.object(Path, 'replace', fail_second), self.assertRaises(OSError):
            pkg.publish(staged, self.dist, ['a', 'b'])
        self.assertEqual((self.dist/'a').read_text(), 'old')
        self.assertEqual((self.dist/'b').read_text(), 'old')

    def test_invalid_binary_never_published(self):
        (self.target/'release/capture-view.exe').write_text('not an executable')
        with self.assertRaises(ValueError):
            self.package()
        self.assertEqual(list(self.dist.iterdir()), [])

    def test_zip_rejects_missing_or_private_entries(self):
        archive = self.root/'bad.zip'
        with zipfile.ZipFile(archive, 'w') as z:
            z.writestr('private.bmp', b'image')
        with self.assertRaises(ValueError):
            pkg.validate_zip(archive, ['CaptureView.exe'])
        with self.assertRaises(ValueError):
            pkg.validate_zip(archive, [])


if __name__ == '__main__':
    unittest.main()
