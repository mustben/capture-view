//! Win32 shell. Re-entrant callbacks post work to the owning message loop.
use std::{cell::RefCell, mem::size_of};
use windows::Win32::Media::DirectShow::IVideoWindow;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{VK_ESCAPE, VK_F5, VK_F11},
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, Result, w},
};
// A cloned COM interface, confined to its creating STA. Borrow ends BEFORE COM calls,
// so nested Win32 size/move loops can resize the preview without aliasing app state.
thread_local! {
    static RENDERER: RefCell<Option<(IVideoWindow,i32,i32)>> = const { RefCell::new(None) };
}
pub fn set_renderer(renderer: Option<(IVideoWindow, i32, i32)>) {
    let previous = RENDERER.with(|slot| slot.replace(renderer));
    // Release COM only after the RefCell borrow has ended (Release can re-enter Win32).
    drop(previous);
}
pub fn resize_renderer(hwnd: HWND) {
    let view = RENDERER.with(|slot| slot.borrow().clone());
    if let Some((video, vw, vh)) = view {
        let mut rect = RECT::default();
        // SAFETY: the interface and window belong to this STA; no TLS borrow survives the call.
        unsafe {
            if GetClientRect(hwnd, &mut rect).is_ok() {
                let (x, y, w, h) = crate::layout::fit(rect.right, rect.bottom, vw, vh);
                if w > 0 && h > 0 {
                    let _ = video.SetWindowPosition(x, y, w, h);
                }
            }
        }
    }
}
pub const TOGGLE: u32 = WM_APP + 2;
pub const KEY: u32 = WM_APP + 3;
pub const REFRESH: u32 = WM_APP + 4;

fn refresh_menu(hwnd: HWND) -> Result<()> {
    // SAFETY: native popup is used on the owner thread, then destroyed even on error.
    unsafe {
        let menu = CreatePopupMenu()?;
        let result = (|| {
            AppendMenuW(menu, MF_STRING, 1, w!("Rafraîchir le flux\tF5"))?;
            let mut point = POINT::default();
            GetCursorPos(&mut point)?;
            if TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                None,
                hwnd,
                None,
            )
            .0 == 1
            {
                PostMessageW(Some(hwnd), REFRESH, WPARAM(0), LPARAM(0))?;
            }
            Ok(())
        })();
        let _ = DestroyMenu(menu);
        result
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: Win32 supplies valid message parameters; no Rust pointers are dereferenced.
    unsafe {
        match message {
            WM_SIZE | WM_EXITSIZEMOVE => {
                resize_renderer(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let _ = PostMessageW(Some(hwnd), TOGGLE, wp, lp);
                LRESULT(0)
            }
            WM_RBUTTONUP | WM_CONTEXTMENU if RENDERER.with(|slot| slot.borrow().is_some()) => {
                // Mouse context messages can follow the forwarded button-up; do not show twice.
                if message == WM_RBUTTONUP || lp.0 == -1 {
                    let _ = refresh_menu(hwnd);
                }
                LRESULT(0)
            }
            WM_KEYDOWN if wp.0 == VK_F5.0 as usize => {
                if lp.0 & (1 << 30) == 0 {
                    let _ = PostMessageW(Some(hwnd), REFRESH, wp, lp);
                }
                LRESULT(0)
            }
            WM_KEYDOWN if wp.0 == VK_F11.0 as usize || wp.0 == VK_ESCAPE.0 as usize => {
                if lp.0 & (1 << 30) == 0 {
                    let _ = PostMessageW(Some(hwnd), KEY, wp, lp);
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                // SAFETY: Windows supplies a RECT valid for this synchronous DPI message.
                let rect = *(lp.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                LRESULT(0)
            }
            WM_CLOSE => {
                // Keep the owner alive until Capture has stopped and detached its renderer.
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wp, lp),
        }
    }
}
pub struct Window {
    pub hwnd: HWND,
    pub fullscreen: bool,
    placement: WINDOWPLACEMENT,
}
impl Window {
    pub fn new() -> Result<Self> {
        // SAFETY: strings are static and window_proc has the correct Win32 ABI.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                style: CS_DBLCLKS | CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                lpszClassName: w!("CaptureViewWindow"),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(windows::core::Error::from_thread());
            }
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class.lpszClassName,
                w!("CaptureView"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                976,
                589,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            Ok(Self {
                hwnd,
                fullscreen: false,
                placement: WINDOWPLACEMENT {
                    length: size_of::<WINDOWPLACEMENT>() as u32,
                    ..Default::default()
                },
            })
        }
    }
    pub fn title(&self, text: &str) -> Result<()> {
        let title: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // SAFETY: title lives through the call; this thread owns hwnd.
        unsafe { SetWindowTextW(self.hwnd, PCWSTR(title.as_ptr())) }
    }
    pub fn show_opening(&self) -> Result<()> {
        self.title("CaptureView — ouverture du boîtier…")?;
        // SAFETY: live window owned by this thread; timer is dispatched by its loop.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            if SetTimer(Some(self.hwnd), 1, 100, None) == 0 {
                return Err(windows::core::Error::from_thread());
            }
        }
        Ok(())
    }
    pub fn hide(&self) {
        // SAFETY: this thread owns the live window; it remains alive until Drop.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }
    pub fn show(&self, audio: bool) -> Result<()> {
        let title: Vec<u16> = format!(
            "CaptureView{}",
            if audio { "" } else { " — audio indisponible" }
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        // SAFETY: title remains live for SetWindowText; hwnd belongs to this thread.
        unsafe {
            SetWindowTextW(self.hwnd, PCWSTR(title.as_ptr()))?;
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            if SetTimer(Some(self.hwnd), 1, 1000, None) == 0 {
                return Err(windows::core::Error::from_thread());
            }
        }
        Ok(())
    }
    pub fn discard_refresh_requests(&self) {
        let mut msg = MSG::default();
        // SAFETY: remove only queued refresh commands for this live owner on its thread.
        unsafe {
            while PeekMessageW(&mut msg, Some(self.hwnd), REFRESH, REFRESH, PM_REMOVE).as_bool() {
                // PeekMessage retrieves WM_QUIT even outside its filter range.
                if msg.message == WM_QUIT {
                    PostQuitMessage(msg.wParam.0 as i32);
                    break;
                }
            }
        }
    }
    pub fn key(&mut self, key: usize) -> Result<()> {
        if key == VK_F11.0 as usize || (key == VK_ESCAPE.0 as usize && self.fullscreen) {
            self.toggle()?;
        }
        Ok(())
    }
    pub fn toggle(&mut self) -> Result<()> {
        // SAFETY: hwnd is alive; saved placement uses the required initialized size.
        unsafe {
            if !self.fullscreen {
                GetWindowPlacement(self.hwnd, &mut self.placement)?;
                let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
                let mut info = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                GetMonitorInfoW(monitor, &mut info).ok()?;
                SetWindowLongPtrW(
                    self.hwnd,
                    GWL_STYLE,
                    (WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN).0 as isize,
                );
                let r = info.rcMonitor;
                SetWindowPos(
                    self.hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_FRAMECHANGED,
                )?;
            } else {
                SetWindowLongPtrW(
                    self.hwnd,
                    GWL_STYLE,
                    (WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN).0 as isize,
                );
                SetWindowPlacement(self.hwnd, &self.placement)?;
                SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                )?;
            }
            self.fullscreen = !self.fullscreen;
            Ok(())
        }
    }
    pub fn client_size(&self) -> Result<(i32, i32)> {
        let mut rect = RECT::default();
        // SAFETY: rect is writable and hwnd is alive.
        unsafe {
            GetClientRect(self.hwnd, &mut rect)?;
        }
        Ok((rect.right, rect.bottom))
    }
    pub fn resize_test(&self) -> Result<()> {
        // SAFETY: resize only our own window for the explicit hardware smoke test.
        unsafe { SetWindowPos(self.hwnd, None, 0, 0, 416, 339, SWP_NOMOVE | SWP_NOZORDER) }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: destroy only our still-live window, on its creating thread.
        unsafe {
            if IsWindow(Some(self.hwnd)).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f5_posts_one_refresh_and_ignores_key_repeat() {
        let window = Window::new().unwrap();
        let mut msg = MSG::default();
        // SAFETY: synchronous input on this test's window/thread; inspect only REFRESH messages.
        unsafe {
            SendMessageW(
                window.hwnd,
                WM_KEYDOWN,
                Some(WPARAM(VK_F5.0 as usize)),
                Some(LPARAM(0)),
            );
            SendMessageW(
                window.hwnd,
                WM_KEYDOWN,
                Some(WPARAM(VK_F5.0 as usize)),
                Some(LPARAM(1 << 30)),
            );
            assert!(
                PeekMessageW(&mut msg, Some(window.hwnd), REFRESH, REFRESH, PM_REMOVE).as_bool()
            );
            assert!(
                !PeekMessageW(&mut msg, Some(window.hwnd), REFRESH, REFRESH, PM_REMOVE).as_bool()
            );
            SendMessageW(
                window.hwnd,
                WM_KEYDOWN,
                Some(WPARAM(VK_F5.0 as usize)),
                Some(LPARAM(0)),
            );
            window.discard_refresh_requests();
            assert!(
                !PeekMessageW(&mut msg, Some(window.hwnd), REFRESH, REFRESH, PM_REMOVE).as_bool()
            );
        }
    }
    #[test]
    fn refresh_coalescing_preserves_close_request() {
        let window = Window::new().unwrap();
        let mut msg = MSG::default();
        // SAFETY: own thread's quit flag; consume it after checking preservation.
        unsafe {
            PostQuitMessage(0);
            window.discard_refresh_requests();
            assert!(PeekMessageW(&mut msg, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool());
            assert_eq!(msg.message, WM_QUIT);
        }
    }
    #[test]
    fn close_request_keeps_owner_alive_for_graph_teardown() {
        let window = Window::new().unwrap();
        // SAFETY: SendMessage targets this test's live window on its owner thread.
        unsafe {
            SendMessageW(window.hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
            assert!(
                IsWindow(Some(window.hwnd)).as_bool(),
                "renderer owner destroyed before graph teardown"
            );
        }
    }
}
