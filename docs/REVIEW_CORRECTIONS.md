# Review corrections — CV-01 through CV-08

Base: `04d903f7bde478c5c71d6cc5f1625873513f3a5f` (`test`). No intervening commits were present when work began.

| ID | Assessment and implementation | Evidence / remaining acceptance |
| --- | --- | --- |
| CV-01 | Confirmed. Per-device property failures log index/HRESULT and continue. Enumerator failures propagate; incomplete absence differs from complete absence; multiple readable matches are rejected. Selected-device bind failures remain visible. | Pure selection tests cover unreadable/unrelated devices, ambiguity, complete/incomplete absence. Selected bind remains outside the recoverable-property closure. Real occupied-card test pending. |
| CV-02 | Confirmed unsupported assumption. Checked DIB parser supports 40-byte BI_RGB headers at 1/4/8/24/32 bpp, including indexed palettes and top-down images. Other headers/compressions/masks are explicitly rejected. Second returned length is checked against supplied capacity before making a slice. Capture failures and resume failures are retained together. | Unit fixtures check offsets, size, pixel preservation, palette, truncation, invalid dimensions/format and overflow. Real captured BMP inspection pending. Driver memory writes themselves remain subject to the COM API contract. |
| CV-03 | Confirmed architectural risk, no hardware hang reproduced. Main thread pumps an opening/status window; separate STA creates its own hidden viewer owner, opens graph and runs viewer. No interfaces or HWND ownership cross threads. | Closing opening shell requests cancellation, keeps status responsive and waits for driver return before graph/owner/COM teardown on viewer STA. Driver calls cannot be forcibly cancelled; no global startup/close bound is promised. Real Windows normal/slow/occupied/close/reacquisition tests pending. |
| CV-04 | Confirmed. Integrated locked release build, Cargo target_directory, explicit optional --target, MZ/PE32+/AMD64/GUI/header/section bounds checks. Both ZIPs and checksums staged and validated before publication; ordinary replacement errors roll back previous files. | Python tests use synthetic PE fixtures, custom paths, licenses, checksums and injected packaging/publication failures. Multi-file replacement is not crash-atomic; no claim of reproducibility against simultaneous external source edits. |
| CV-05 | Confirmed after checking locked windows 0.62 binding: GetState maps successful HRESULT to the numeric state. Local vtable call retains HRESULT; only S_OK plus Running confirms startup. Intermediate/unknown status is not running. | Zero-wait polling, separate graph errors and 10-second confirmation deadline after Run returns. Pure status tests. Confirmation does not prove HDMI activity, audible sound or synchronization. |
| CV-06 | Confirmed. Hardware-free policy/parser/PE/packaging tests added. UI waits poll conditions and detect process exit, select CaptureViewWindow with renderer belonging to launched PID. | UI/hardware tests remain opt-in; actual Windows hardware validation pending. |
| CV-07 | To reproduce. Existing mixed-DPI behavior deliberately unchanged: no evidence here justifies changing saved placement/fullscreen handling. | Manual two-monitor checklist in TESTING.md; no mixed-DPI hardware available in this environment. |
| CV-08 | Confirmed maintenance improvement. Rust 1.93.0 pinned, actions pinned to verified v4.2.2/v4.6.2 commit references; locked builds, read-only contents permission, 20-minute job deadline. | Update toolchain/action pins through reviewed changes and run Windows CI. Source archive includes toolchain and new tests/docs. |

## Opening cancellation and simulation

The visible status shell and viewer use different UI threads. A single viewer STA owns every graph interface and renderer clone, its owner HWND and COM apartment. The status shell never controls COM. After opening returns, cancellation is checked before showing the viewer; a cancellation race is also checked by the viewer message loop. Errors are logged/reported even when a close request overlaps a driver error. A stuck driver can keep the process alive with a responsive waiting status; terminating that process externally forfeits graceful teardown guarantees.

For a reproducible slow-opening UI check without assuming a slow driver:

```powershell
$env:CAPTUREVIEW_TEST_OPEN_DELAY_MS = '10000'
.\target\release\capture-view.exe
Remove-Item Env:CAPTUREVIEW_TEST_OPEN_DELAY_MS
```

The explicit diagnostic hook delays up to 60 seconds before opening, checks cancellation every 20 ms, and creates no fake media. Close during the delay: process should exit without opening the device. Separately test closing during an actual slow driver call and verify that the status remains responsive until the call returns. Once viewer appears, status shell is hidden; only one viewer is visible during playback.

## Validation record

Local checks executed: Rust 1.93.0 formatting passed; cargo check and Clippy with warnings denied passed for x86_64-pc-windows-msvc (all targets, locked). Eleven pure Rust tests ran successfully through native rustc test harnesses (DIB: 2; selection: 2; graph status/deadline: 2; layout: 5). Eight Python packaging tests passed; the Windows-only opening test was skipped on Linux. See Windows Actions for native build/test/package/opening-shell execution. Linux-hosted cross-checks cannot validate Win32 execution, the card, HDMI, audio, Bluetooth, latency, synchronization or multi-monitor DPI. Previous validation in TESTING.md is historical evidence, not a test run for these corrections.
