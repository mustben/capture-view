//! One native window and graph on one STA; this loop owns all application state.
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
        // SAFETY: called once on the main thread before creating COM objects.
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
fn run(smoke: bool, log: &mut Option<File>) -> Result<(), Box<dyn std::error::Error>> {
    let _com = ComApartment::new()?;
    let mut window = window::Window::new()?;
    // Reverse drop order guarantees graph -> window -> COM apartment.
    let mut graph = capture::Capture::open(window.hwnd, &mut |s| log_line(log, s))?;
    window.show(graph.audio)?;
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
        // SAFETY: MSG is writable; the main STA pumps its own messages.
        let status = unsafe { GetMessageW(&mut msg, None, 0, 0).0 };
        if status == -1 {
            return Err(windows::core::Error::from_thread().into());
        }
        if status == 0 {
            break;
        }
        match msg.message {
            window::TOGGLE if msg.hwnd == window.hwnd => window.toggle()?,
            window::KEY if msg.hwnd == window.hwnd => window.key(msg.wParam.0)?,
            WM_TIMER if msg.hwnd == window.hwnd && msg.wParam.0 == 1 => {
                if let Some(error) = graph.drain_error() {
                    return Err(error.into());
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
