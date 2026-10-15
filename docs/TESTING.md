# Testing and evidence

## Reproducible checks (Windows x64)

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
```

Unit tests exercise small/portrait/4:3 layouts, invalid dimensions, and the native close-before-destroy lifetime regression. They do not simulate capture hardware.

## Real hardware smoke test

Requires the manufacturer's driver, exactly one AVerMedia DirectShow card and a working Windows audio endpoint. Close other capture applications. The test visibly opens/resizes the window and enters fullscreen briefly. It is opt-in and must not run in unattended CI.

```powershell
Copy-Item .\target\release\capture-view.exe .\CaptureView.exe
python .\integration_test.py
```

The script runs `CaptureView.exe --smoke-test` with a 40-second timeout and checks the exit code plus all report fields. The app connects video/audio, queries running state, resizes, enters/exits fullscreen, verifies client size restoration, pauses briefly to retrieve a renderer DIB, resumes, requests a normal WM_CLOSE and tears down.

Artifacts in the working directory:
- `smoke-result.json`: machine-readable checks.
- `smoke-frame.bmp`: actual frame returned by the renderer, potentially private. **Never commit or upload without inspecting it.**
- `%LOCALAPPDATA%\CaptureView\last-run.log`: diagnostic information.

The `audio` field only means that the audio graph connection succeeded. It does **not** mean that audible sound was present. The `frame` field means a real renderer image was retrieved, including possible No Signal screens; it does **not** mean a live HDMI source was verified. The `closed` field records graph/window teardown returning; the wrapper's successful process exit is the additional lifetime check.

## Native UI regression (real hardware, opt-in)

After building and copying the executable as above, run:

```powershell
python scripts/ui_regression.py
```

This launches only its own viewer process. It starts the real Windows modal sizing loop using SC_SIZE, sends resize keys and verifies that the child renderer fits the new client size. It then sends a double-click to the actual video child (testing message-drain forwarding), verifies F11/Escape and exact client-size restoration, requests normal WM_CLOSE and checks a clean process exit. The test has cleanup for its own process only. Do not run it while another application is using the card.

## Initial local validation

- Windows 11 x64, AVerMedia Live Gamer EXTREME GC550.
- Driver exposes YUY2 up to 1920×1080 at approximately 60 fps and an audio pin on the same source filter (48 kHz stereo observed separately using FFmpeg as an investigative tool, **not** an app dependency).
- Native release build, Clippy with warnings denied and unit tests passed.
- Hardware smoke checks passed, including retrieval of the GC550's No Signal image.
- Native UI regression passed: modal resize from 960×550 to 1092×638 client pixels, renderer 1092×614, video-child double-click, F11/Escape, exact size restoration and normal close. The hardware smoke test passed again immediately afterwards, demonstrating device reacquisition by a fresh process.
- Following the initial automated tests, the original user confirmed that the application works correctly in real use on their setup. This is a user report, not an instrumented AV/latency measurement.
- Long-duration lip synchronization, unplug/replug, mixed-DPI monitors and end-to-end latency remain separate manual acceptance tests.

Do not promote this to broad model compatibility or invent latency benchmarks. A future compatibility contribution should state exact device model, driver, Windows build, USB connection, video format and whether both picture and audible sound were tested.

## Manual acceptance checklist

- [ ] Active HDMI picture (motion) and audible sound, correct channels.
- [ ] No progressive AV drift during a 30-minute session.
- [ ] Small/large window; no crop or stretch; minimize/restore.
- [ ] F11, double-click, Escape, normal close and Alt+F4.
- [ ] Multiple displays and DPI scales; fullscreen restores previous placement.
- [ ] Close then reopen; OBS can reacquire the device after CaptureView closes.
- [ ] Explicit, recoverable error for unplugged/in-use devices; no leftover process.
