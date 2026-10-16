//! Responsive opening shell plus a viewer STA which exclusively owns its COM graph.
use crate::{capture, window};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use windows::Win32::{System::Com::*, UI::WindowsAndMessaging::*};
use windows::core::w;
struct ComApartment;
impl ComApartment {
    fn new() -> windows::core::Result<Self> {
        // SAFETY: called once on the viewer thread before creating COM objects.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: graph interfaces have already dropped on this same thread.
        unsafe {
            CoUninitialize();
        }
    }
}
fn log_line(log: &mut Option<File>, text: &str) {
    if let Some(file) = log {
        let _ = writeln!(file, "{text}");
    }
}
enum ViewerStatus {
    Refreshing,
    Ready,
}
fn run_viewer(
    smoke: bool,
    log: &mut Option<File>,
    cancelled: &std::sync::atomic::AtomicBool,
    notify: std::sync::mpsc::Sender<ViewerStatus>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _com = ComApartment::new()?;
    let mut window = window::Window::new()?;
    // Reverse drop order guarantees graph -> window -> COM apartment.
    // Explicit test hook; no hardware is simulated by this delay.
    if let Ok(delay) = std::env::var("CAPTUREVIEW_TEST_OPEN_DELAY_MS") {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(delay.parse::<u64>()?.min(60_000));
        while std::time::Instant::now() < deadline {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(());
    }
    let mut graph = capture::Capture::open(window.hwnd, &mut |s| log_line(log, s))?;
    if cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(()); // graph -> hidden viewer owner -> COM, all on this STA.
    }
    window.show(graph.audio)?;
    let _ = notify.send(ViewerStatus::Ready);
    let mut startup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut confirmed = false;
    let mut refreshing = false;
    let (w, h) = window.client_size()?;
    graph.resize(w, h)?;
    let mut msg = MSG::default();
    let mut step = 0;
    let mut checks = [
        true,
        graph.audio,
        graph.running(),
        false,
        false,
        false,
        false,
        false,
    ];
    let mut original = (0, 0);
    loop {
        // SAFETY: MSG is writable; the viewer STA pumps its own messages.
        let status = unsafe { GetMessageW(&mut msg, None, 0, 0).0 };
        if status == -1 {
            return Err(windows::core::Error::from_thread().into());
        }
        if status == 0 {
            break;
        }
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        match msg.message {
            window::TOGGLE if msg.hwnd == window.hwnd => window.toggle()?,
            window::KEY if msg.hwnd == window.hwnd => window.key(msg.wParam.0)?,
            window::REFRESH if msg.hwnd == window.hwnd => {
                if !confirmed {
                    window.finish_refresh_request();
                    continue;
                }
                log_line(log, "Refresh requested: rebuilding video and audio graph.");
                // Keep geometry/fullscreen and HWND ownership; release the entire old graph first.
                window.hide();
                let _ = notify.send(ViewerStatus::Refreshing);
                drop(graph);
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                graph = capture::Capture::open(window.hwnd, &mut |s| log_line(log, s))
                    .map_err(|error| format!("Impossible de rafraîchir le flux : {error}"))?;
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                // Callback-side pending flag suppresses additional requests throughout reopen.
                window.finish_refresh_request();
                let (w, h) = window.client_size()?;
                graph.resize(w, h)?;
                window.show(graph.audio)?;
                window.title("CaptureView — redémarrage du flux…")?;
                let _ = notify.send(ViewerStatus::Ready);
                confirmed = false;
                refreshing = true;
                startup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            }
            WM_TIMER if msg.hwnd == window.hwnd && msg.wParam.0 == 1 => {
                if let Some(error) = graph.drain_error() {
                    return Err(error.into());
                }
                if !confirmed {
                    let state = graph.state()?;
                    let expired = std::time::Instant::now() >= startup_deadline;
                    if crate::graph_state::startup_confirmed(&state, expired)
                        .map_err(|error| format!("{error}: {state:?}"))?
                    {
                        confirmed = true;
                        checks[2] = true;
                        log_line(
                            log,
                            "Graph startup confirmed (S_OK, Running); HDMI activity/audio audibility not measured.",
                        );
                        if refreshing {
                            log_line(
                                log,
                                "Refresh confirmed: new video/audio graph is Running; AV synchronization not measured.",
                            );
                            refreshing = false;
                        }
                        window.show(graph.audio)?;
                    } else {
                        window.title("CaptureView — démarrage vidéo en cours…")?;
                        continue;
                    }
                }
                if smoke {
                    step += 1;
                    match step {
                        1 => {
                            window.resize_test()?;
                            original = window.client_size()?;
                            checks[3] = original.0 > 0 && original.0 < 500;
                        }
                        2 => {
                            window.toggle()?;
                            checks[4] = window.fullscreen && window.client_size()?.0 > original.0;
                        }
                        3 => {
                            window.toggle()?;
                            checks[5] = !window.fullscreen && window.client_size()? == original;
                        }
                        4 => match graph.save_frame(std::path::Path::new("smoke-frame.bmp")) {
                            Ok(()) => checks[6] = true,
                            Err(e) => log_line(log, &format!("Frame capture failed: {e}")),
                        },
                        5 => {
                            checks[2] &= graph.running();
                            // Exercise the same WM_CLOSE path as the title-bar button/Alt+F4.
                            // SAFETY: this is our live window and asynchronous close request.
                            unsafe {
                                PostMessageW(
                                    Some(window.hwnd),
                                    WM_CLOSE,
                                    windows::Win32::Foundation::WPARAM(0),
                                    windows::Win32::Foundation::LPARAM(0),
                                )?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                // SAFETY: dispatch the message returned by GetMessageW.
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
    log_line(
        log,
        &format!(
            "Closing {} ({}x{})",
            graph.device_name, graph.width, graph.height
        ),
    );
    drop(graph);
    drop(window);
    checks[7] = true;
    if smoke {
        let names = [
            "video",
            "audio",
            "running",
            "resize",
            "fullscreen",
            "restore",
            "frame",
            "closed",
        ];
        let entries: Vec<String> = names
            .iter()
            .zip(checks)
            .map(|(n, v)| format!("\"{n}\":{v}"))
            .collect();
        std::fs::write("smoke-result.json", format!("{{{}}}", entries.join(",")))?;
        if !checks.iter().all(|v| *v) {
            return Err(
                "Hardware smoke test incomplete; see smoke-result.json and last-run.log".into(),
            );
        }
    }
    Ok(())
}
fn run(smoke: bool, log: &mut Option<File>) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let opening = window::Window::new()?;
    opening.show_opening()?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let mut worker_log = log.as_ref().and_then(|file| file.try_clone().ok());
    // No HWND or COM interface crosses threads. The viewer creates its own hidden owner.
    let worker = std::thread::spawn(move || {
        let result = run_viewer(smoke, &mut worker_log, &worker_cancelled, ready_tx)
            .map_err(|error| error.to_string());
        let _ = done_tx.send(result);
    });
    let mut msg = MSG::default();
    let result = loop {
        for status in ready_rx.try_iter() {
            if !cancelled.load(Ordering::Acquire) {
                match status {
                    ViewerStatus::Refreshing => {
                        let _ = opening.show_opening();
                        let _ = opening.title("CaptureView — rafraîchissement du flux…");
                    }
                    ViewerStatus::Ready => opening.hide(),
                }
            }
        }
        match done_rx.try_recv() {
            Ok(result) => break result,
            Err(mpsc::TryRecvError::Disconnected) => {
                break Err("Viewer thread stopped unexpectedly".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        // SAFETY: writable MSG; opening thread continues pumping throughout driver calls.
        let status = unsafe { GetMessageW(&mut msg, None, 0, 0).0 };
        if status == -1 {
            cancelled.store(true, Ordering::Release);
            // Exceptional message-loop failure: wait for orderly worker-owned COM teardown.
            break Err(windows::core::Error::from_thread().to_string());
        }
        if status == 0 {
            cancelled.store(true, Ordering::Release);
            let _ = opening.title("CaptureView — fermeture demandée, attente du pilote…");
            // WM_CLOSE does not destroy the owner. No forced termination of COM/driver calls.
            continue;
        }
        // Keep the status shell independent from viewer controls.
        if msg.message != window::TOGGLE
            && msg.message != window::KEY
            && msg.message != window::REFRESH
        {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    };
    cancelled.store(true, Ordering::Release);
    worker.join().map_err(|_| "Viewer thread panicked")?;
    result.map_err(Into::into)
}
pub fn main() {
    let smoke = std::env::args().any(|a| a == "--smoke-test");
    let dir =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("CaptureView");
    let _ = std::fs::create_dir_all(&dir);
    let mut log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(dir.join("last-run.log"))
        .ok();
    log_line(&mut log, concat!("CaptureView ", env!("CARGO_PKG_VERSION")));
    if let Err(error) = run(smoke, &mut log) {
        let text = format!(
            "{error}\n\nFermez OBS/RECentral s'ils utilisent le boîtier, puis relancez CaptureView.\nJournal : {}",
            dir.join("last-run.log").display()
        );
        log_line(&mut log, &text);
        if !smoke {
            let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            // SAFETY: the nul-terminated buffer lives until MessageBox returns.
            unsafe {
                MessageBoxW(
                    None,
                    windows::core::PCWSTR(text.as_ptr()),
                    w!("CaptureView"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        std::process::exit(1);
    }
}
