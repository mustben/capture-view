# Testing and evidence

## Reproducible checks (Windows x64)

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
python -m unittest discover -s scripts -p "test_*.py"
python scripts/package.py
```

Unit tests exercise layouts, unreadable/ambiguous/missing-device selection, checked BMP headers/palettes/bounds, intermediate HRESULTs and confirmation deadlines, and the native close-before-destroy lifetime regression. Python tests inject malformed PE files and packaging/publication failures and verify Cargo artifact paths, licenses, ZIP entries and checksums. CI separately tests a visible, responsive opening shell and cancellation during a simulated delay before driver access. They do not simulate capture hardware.

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

## Acceptance for review corrections

Use Rust 1.93.0 from `rust-toolchain.toml`. The per-ID assessment and slow-opening simulation procedure are in [REVIEW_CORRECTIONS.md](REVIEW_CORRECTIONS.md). To run the CI opening-shell test manually on Windows without capture hardware:

```powershell
$env:CAPTUREVIEW_TEST_EXE = (Resolve-Path target/release/capture-view.exe).Path
python scripts/test_opening.py
Remove-Item Env:CAPTUREVIEW_TEST_EXE
```

This proves message processing and cancellation during a simulated pre-driver delay only. Test normal opening, no device, occupied device, close during actual driver opening, and device reacquisition separately on real hardware. Inspect the generated BMP locally; do not upload the private frame.

For mixed DPI, use two monitors with different scale factors (for example 100% and 150%): move the window in both directions, enter fullscreen on each, change scale while fullscreen, leave fullscreen and verify usable restored position and size. Record Windows build, scales and observed geometry. No mixed-DPI defect has been reproduced during this correction; that path is unchanged.

## Manual refresh acceptance (real hardware)

F5 and right-click → Rafraîchir le flux rebuild both video and audio. The native UI regression also sends F5 through the actual video child, waits for the new graph's confirmation in the local log, reacquires its renderer HWND, and checks windowed/fullscreen placement restoration. This remains hardware-only and opt-in.

- Start with live HDMI motion and audible audio. If drift occurs, note it, press F5 and check both picture and audible sound afterwards. Graph confirmation alone cannot verify synchronization.
- Repeat using the right-click menu. Verify aspect ratio, window size, fullscreen and the original restored placement.
- Test several manual refreshes, held F5 (no auto-repeat reopen loop), an occupied/disconnected device on reopen, and close during a slow refresh. A blocked driver must return before graceful cleanup completes.
- Run at least 30 minutes after refresh to assess whether drift returns. Record driver/output device and the observations without publishing private media.

The automated Windows shell tests cover shortcut routing and key-repeat suppression without a device. They do not demonstrate AV resynchronization; no real-card refresh test was run in the hosted development environment.
