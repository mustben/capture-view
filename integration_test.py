import pathlib, subprocess, json
p=pathlib.Path(__file__).parent
exe=p/'CaptureView.exe'
assert exe.exists(), 'Application CaptureView.exe not built yet'
(p/'smoke-result.json').unlink(missing_ok=True)
r=subprocess.run([str(exe),'--smoke-test'],cwd=p,timeout=40)
assert r.returncode==0, f'Application failed: {r.returncode}'
d=json.loads((p/'smoke-result.json').read_text())
for key in ['video','audio','running','resize','fullscreen','restore','frame','closed']:
    assert d.get(key) is True, f'{key} failed: {d}'
print('PASS: real GC550 video/audio graph, running, resize, fullscreen, restore, frame capture, clean close')
print(d)
