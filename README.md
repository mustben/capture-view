# CaptureView

[![Windows Rust checks](https://github.com/mustben/capture-view/actions/workflows/ci.yml/badge.svg?branch=test)](https://github.com/mustben/capture-view/actions/workflows/ci.yml)

[Download for Windows](https://github.com/mustben/capture-view/releases/latest)

**A capture-card viewer, not a streaming studio.** One window. Picture and sound. Resize it or double-click for fullscreen.

[Documentation française](docs/README.fr.md) · [Architecture](docs/ARCHITECTURE.md) · [Testing](docs/TESTING.md) · [Contributing](CONTRIBUTING.md)

CaptureView is a small native Windows application written in Rust. It connects an AVerMedia DirectShow capture device directly to Windows video/audio renderers. No OBS, RECentral, VLC, FFmpeg, Electron or browser is required to run it. The capture-card driver is still required.

## Use

Download and extract **CaptureView-windows-x64.zip** from [Releases](https://github.com/mustben/capture-view/releases/latest). No installation of CaptureView itself is needed.

1. Install the appropriate AVerMedia driver, plug the device into USB 3 and connect the HDMI source.
2. Close OBS/RECentral or any application already using that device.
3. Launch `CaptureView.exe`.

- Drag the window edges to resize. The image keeps its aspect ratio.
- Double-click the picture or press **F11** to toggle fullscreen.
- **Escape** leaves fullscreen. **Alt+F4** or the close button exits.
- Audio goes to the default Windows playback device. Adjust volume in Windows.

No settings panel, recording, network access, telemetry, administrator privileges or startup service. Only one AVerMedia device may be connected. The program writes a small diagnostic log to `%LOCALAPPDATA%\CaptureView\last-run.log`, replaced on each launch. Normal playback saves no images or audio.

## Compatibility and honest limits

| Item | Status |
| --- | --- |
| Windows 11 x64 + AVerMedia Live Gamer EXTREME GC550 | Device opening, 1920×1080 graph, audio-pin connection and UI tested |
| Live use | Original user reports the app works correctly on their GC550 setup; AV synchronization/latency have not been instrumented |
| Other AVerMedia DirectShow cards | May work, **not tested** |
| Windows 10 x64 | Intended platform, **not tested** |
| Linux / macOS / ARM64 | Not supported |

The app selects the single DirectShow video device whose friendly name contains `AVerMedia` (case-insensitive). Audio must be exposed on that same filter, as it is on the GC550. If audio cannot connect, video remains available and the title indicates the problem. The driver's default mode is retained; resolution/frame rate are logged after negotiation rather than assumed. No general capability selector is implemented.

The video path is designed to avoid application-side frame copies or encoding. This is **not a zero-latency guarantee**: USB, the driver, format conversion, renderer and audio device still add latency and consume memory. VMR9 is a legacy Windows renderer chosen for simplicity and compatibility. Memory use includes its buffers and driver allocations; it is not just the size of the Rust executable.

HDCP-protected sources are not supported or bypassed. Unplugging or changing devices may require closing and reopening the app. There is no automatic reconnection or input switching.

## Build from source

Prerequisites: Windows x64, stable Rust **1.93 or later**, and Visual Studio Build Tools with **Desktop development with C++** plus a Windows SDK. Use the `x86_64-pc-windows-msvc` toolchain.

```powershell
cargo build --release --locked
.\target\release\capture-view.exe
```

The result is a single executable. The repository statically links the MSVC runtime and embeds a per-monitor-DPI-aware application manifest. Windows system components and the manufacturer's installed driver are not bundled.

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

For the optional hardware test, see [TESTING.md](docs/TESTING.md). CI performs static checks and unit tests, **not** capture-card validation.

## Package for distribution

After a release build, run `python scripts/package.py` (Python 3.10+). It produces a portable binary ZIP with dependency license texts, a source ZIP from an explicit allowlist, and SHA256SUMS.txt under `dist/`. Diagnostic frames, logs and build caches are excluded. Nothing is uploaded or published by this command.

## Troubleshooting

- **No Signal** inside the picture: the device is producing that image. Check HDMI source power, connection and HDCP.
- **No AVerMedia found**: check USB, driver installation and Device Manager.
- **Opening failed**: close other applications using the card, then retry.
- **No sound**: check the default Windows output/volume and HDMI source audio. Consult the log; an audio graph connection alone does not prove an audible signal.
- **Windows SmartScreen warning**: local/community builds are unsigned. Review and build the source if needed; do not disable security protections globally.

## License and attribution

MIT. CaptureView is independent and is not affiliated with or endorsed by AVerMedia. Brand and product names belong to their owners. No proprietary application code, firmware or drivers are included. The implementation uses documented Windows COM/DirectShow APIs; inspecting the device's exposed interfaces was sufficient, so no proprietary binary decompilation was necessary.

Windows API bindings: the Microsoft `windows` crate, licensed MIT OR Apache-2.0; exact dependencies are recorded in `Cargo.lock`. See [THIRD_PARTY.md](THIRD_PARTY.md).
