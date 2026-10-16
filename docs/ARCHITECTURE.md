# Architecture

## Data path

```text
GC550 USB driver (DirectShow source filter)
  ├─ video capture pin → preview/Smart Tee → converter if required → VMR9 child window
  └─ audio output pin → Windows default audio renderer
```

`ICaptureGraphBuilder2::RenderStream` constructs the required connections. Windows may insert converters and renderer buffers. Rust does not run a per-frame processing loop, re-encode, resample manually or send media over a socket. The only application-side image copy is the explicit diagnostic snapshot in `--smoke-test` mode.

## Modules

- `main.rs`: subsystem declaration and entry point.
- `app.rs`: opening shell, viewer STA lifetime, graph ownership, event loops, diagnostic logging and hardware smoke-test orchestration.
- `capture.rs`: device discovery, rendering graph, diagnostic snapshot, DirectShow event handling and RAII teardown.
- `window.rs`: native window, aspect-independent shell, fullscreen/placement, keyboard and DPI messages.
- `layout.rs`, `selection.rs`, `graph_state.rs`, `dib.rs`: pure layout, device selection, HRESULT/state classification and checked diagnostic BMP conversion, tested without hardware.
- `build.rs` / `app.manifest`: embed asInvoker and PerMonitorV2 manifest; no installer or elevation.
- `.cargo/config.toml`: static MSVC CRT for the supported x64 target.

## Threading and safety

A main UI thread pumps the visible opening/status shell while one viewer STA thread creates its own hidden window and all COM interfaces. No COM wrappers or window ownership move between threads. Once initialization returns, the viewer checks cancellation, shows its window and signals the status shell to hide. The viewer STA then pumps its own message loop; DirectShow manages driver/renderer threads. The status shell continues pumping until the viewer finishes. Closing during opening sets an atomic cancellation request: cleanup waits for driver calls to return, then stops/detaches the graph, destroys its owner and uninitializes COM on the viewer STA. No forced COM-thread termination or global driver-call timeout is promised. See REVIEW_CORRECTIONS.md for the slow-opening test hook.

The window callback never borrows the main application state or dereferences a Rust application pointer. Toggle/key work is posted back to the loop. Resizing is performed synchronously, including inside Windows' modal size/move loop, using a thread-local clone of IVideoWindow and the display aspect ratio. All RefCell borrows end before any COM call or Release, allowing re-entrant callbacks safely. The clone is cleared before graph teardown. Windows-supplied DPI RECT data is consumed synchronously. The viewer loop remains the sole owner of the graph and fullscreen state. Startup is confirmed only by S_OK plus Running using a localized generated-vtable call, because the high-level GetState binding discards positive HRESULT statuses. Zero-wait polls have a 10-second confirmation deadline after Run returns; that deadline does not bound Run or earlier driver calls. Only the application's own timer messages are intercepted; auxiliary COM window timers are dispatched normally.

Unsafe blocks are localized around FFI and documented with safety assumptions. No custom media-type allocation is required: the device default is used. The Capture RAII owner exists before attaching the renderer, so initialization failures also detach it. WM_CLOSE requests loop termination without destroying the owner. Teardown clears the thread-local renderer clone, stops the graph, hides/detaches the child renderer, drops interfaces, destroys the window, then uninitializes COM. EC_VIDEO_SIZE_CHANGED refreshes dimensions/preferred display aspect; device removal and reappearance events are distinguished. Hotplug recovery is still a manual relaunch, not a supported reconnect state machine.

## Deliberate scope

One AVerMedia source, one default Windows audio output, aspect-preserving preview. No networking, telemetry, firmware access, elevated operations, background service or persistent user preferences. Log file is overwritten each run. Friendly-name matching is intentionally conservative: zero or multiple matching cards is an explicit error, not arbitrary selection.

No claim of zero-copy inside Windows or the vendor driver, nor of measured end-to-end latency. VMR9 is older than DXGI/modern Media Foundation renderers, but keeps dependencies and implementation size small for this DirectShow-only device. Changing renderers needs real hardware/latency measurements, not assumptions about language speed.

Diagnostic snapshots support checked BITMAPINFOHEADER/BI_RGB DIBs at 1/4/8/24/32 bits, including palettes. Masks, compressed formats and extended headers are explicitly rejected; this restriction affects only snapshots. Playback resume is attempted on every snapshot path and combined errors retain both capture and resume failures.
