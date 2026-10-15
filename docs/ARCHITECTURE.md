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
- `app.rs`: COM STA lifetime, graph ownership, event loop, bounded diagnostic logging and hardware smoke-test orchestration.
- `capture.rs`: device discovery, rendering graph, diagnostic snapshot, DirectShow event handling and RAII teardown.
- `window.rs`: native window, aspect-independent shell, fullscreen/placement, keyboard and DPI messages.
- `layout.rs`: pure aspect-preserving fit calculation, unit tested without hardware.
- `build.rs` / `app.manifest`: embed asInvoker and PerMonitorV2 manifest; no installer or elevation.
- `.cargo/config.toml`: static MSVC CRT for the supported x64 target.

## Threading and safety

All COM interfaces are created and used on the main STA thread, which pumps Windows messages. There is no custom worker pool. Driver and renderer threads are managed by DirectShow.

The window callback never borrows the main application state or dereferences a Rust application pointer. Toggle/key work is posted back to the loop. Resizing is performed synchronously, including inside Windows' modal size/move loop, using a thread-local clone of IVideoWindow and the display aspect ratio. All RefCell borrows end before any COM call or Release, allowing re-entrant callbacks safely. The clone is cleared before graph teardown. Windows-supplied DPI RECT data is consumed synchronously. The loop remains the sole owner of the graph and fullscreen state. Only the application's own timer messages are intercepted; auxiliary COM window timers are dispatched normally.

Unsafe blocks are localized around FFI and documented with safety assumptions. No custom media-type allocation is required: the device default is used. The Capture RAII owner exists before attaching the renderer, so initialization failures also detach it. WM_CLOSE requests loop termination without destroying the owner. Teardown clears the thread-local renderer clone, stops the graph, hides/detaches the child renderer, drops interfaces, destroys the window, then uninitializes COM. EC_VIDEO_SIZE_CHANGED refreshes dimensions/preferred display aspect; device removal and reappearance events are distinguished. Hotplug recovery is still a manual relaunch, not a supported reconnect state machine.

## Deliberate scope

One AVerMedia source, one default Windows audio output, aspect-preserving preview. No networking, telemetry, firmware access, elevated operations, background service or persistent user preferences. Log file is overwritten each run. Friendly-name matching is intentionally conservative: zero or multiple matching cards is an explicit error, not arbitrary selection.

No claim of zero-copy inside Windows or the vendor driver, nor of measured end-to-end latency. VMR9 is older than DXGI/modern Media Foundation renderers, but keeps dependencies and implementation size small for this DirectShow-only device. Changing renderers needs real hardware/latency measurements, not assumptions about language speed.
