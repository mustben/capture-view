//! Owns the DirectShow graph. No video frames pass through Rust during playback.
//! COM calls stay on the main STA thread; interface wrappers release references on drop.
use std::ptr;
use windows::{
    Win32::{
        Foundation::HWND,
        Media::{DirectShow::*, MediaFoundation::*},
        System::{Com::StructuredStorage::IPropertyBag, Com::*, Variant::VARIANT},
    },
    core::{BSTR, Interface, Result, w},
};

pub struct Capture {
    hwnd: HWND,
    graph: IGraphBuilder,
    control: IMediaControl,
    video: IVideoWindow,
    basic: IBasicVideo,
    pub width: i32,
    pub height: i32,
    pub device_name: String,
    pub audio: bool,
}

impl Capture {
    pub fn open(hwnd: HWND, log: &mut dyn FnMut(&str)) -> Result<Self> {
        // SAFETY: the caller initializes COM STA and keeps hwnd alive until graph teardown.
        unsafe {
            let graph: IGraphBuilder =
                CoCreateInstance(&CLSID_FilterGraph, None, CLSCTX_INPROC_SERVER)?;
            let builder: ICaptureGraphBuilder2 =
                CoCreateInstance(&CLSID_CaptureGraphBuilder2, None, CLSCTX_INPROC_SERVER)?;
            builder.SetFiltergraph(&graph)?;
            let (source, device_name) = find_device()?;
            log(&format!("Device: {device_name}"));
            graph.AddFilter(&source, w!("Capture device"))?;
            // Retain the driver default; do not guess modes or frame rates.
            let renderer: IBaseFilter =
                CoCreateInstance(&CLSID_VideoMixingRenderer9, None, CLSCTX_INPROC_SERVER)?;
            graph.AddFilter(&renderer, w!("Video renderer"))?;
            builder.RenderStream(
                Some(&PIN_CATEGORY_PREVIEW),
                &MEDIATYPE_Video,
                &source,
                None,
                &renderer,
            )?;
            log("Video preview connected (VMR9).");
            // GC550 exposes its audio pin on the VIDEO source filter, not as a microphone.
            let audio = match builder.RenderStream(None, &MEDIATYPE_Audio, &source, None, None) {
                Ok(()) => {
                    log("Audio connected to the default Windows output.");
                    true
                }
                Err(e) => {
                    log(&format!("Audio unavailable: {e}"));
                    false
                }
            };
            let video: IVideoWindow = graph.cast()?;
            let basic: IBasicVideo = graph.cast()?;
            let (mut width, mut height) = (0, 0);
            basic.GetVideoSize(&mut width, &mut height)?;
            if width <= 0 || height <= 0 {
                return Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_FAIL,
                ));
            }
            log(&format!(
                "Negotiated video: {width}x{height}; frame interval: {:.6} s",
                basic.AvgTimePerFrame().unwrap_or(0.0)
            ));
            let control: IMediaControl = graph.cast()?;
            // Construct the owner before Run, so errors also detach the renderer and stop.
            let result = Self {
                hwnd,
                graph,
                control,
                video,
                basic,
                width,
                height,
                device_name,
                audio,
            };
            result.video.SetAutoShow(0)?;
            result.video.SetOwner(hwnd.0 as isize)?;
            result
                .video
                .SetWindowStyle(0x40000000 | 0x04000000 | 0x02000000)?;
            result.video.SetMessageDrain(hwnd.0 as isize)?;
            result.attach_resize();
            result.video.SetVisible(-1)?;
            result.control.Run()?;
            log(&format!(
                "Graph state: {} (2 = running)",
                result.control.GetState(3000)?
            ));
            Ok(result)
        }
    }
    fn aspect(&self) -> (i32, i32) {
        let (mut x, mut y) = (0, 0);
        // SAFETY: optional interface is queried and used on the graph's STA.
        unsafe {
            if let Ok(basic) = self.basic.cast::<IBasicVideo2>()
                && basic.GetPreferredAspectRatio(&mut x, &mut y).is_ok()
                && x > 0
                && y > 0
            {
                return (x, y);
            }
        }
        (self.width, self.height)
    }
    fn attach_resize(&self) {
        let (x, y) = self.aspect();
        crate::window::set_renderer(Some((self.video.clone(), x, y)));
        crate::window::resize_renderer(self.hwnd);
    }
    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        let (vw, vh) = self.aspect();
        let (x, y, w, h) = crate::layout::fit(width, height, vw, vh);
        if w == 0 || h == 0 {
            return Ok(());
        }
        // SAFETY: video is a live COM interface on its creating STA thread.
        unsafe { self.video.SetWindowPosition(x, y, w, h) }
    }
    pub fn running(&self) -> bool {
        // SAFETY: queried on the creating STA thread.
        unsafe { self.control.GetState(1000).is_ok_and(|s| s == 2) }
    }
    pub fn save_frame(
        &self,
        path: &std::path::Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Diagnostic only. Playback itself never allocates or copies a frame in Rust.
        // SAFETY: the renderer is paused; the provided aligned buffer has the reported size.
        unsafe {
            self.control.Pause()?;
            let _ = self.control.GetState(2000);
            let capture = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
                let mut size = 0;
                self.basic.GetCurrentImage(&mut size, ptr::null_mut())?;
                if !(40..=128 * 1024 * 1024).contains(&size) {
                    return Err("Invalid DIB size".into());
                }
                let mut data = vec![0i32; (size as usize).div_ceil(4)];
                self.basic.GetCurrentImage(&mut size, data.as_mut_ptr())?;
                let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), size as usize);
                let mut bmp = Vec::with_capacity(size as usize + 14);
                bmp.extend_from_slice(b"BM");
                bmp.extend_from_slice(&(size as u32 + 14).to_le_bytes());
                bmp.extend_from_slice(&[0; 4]);
                // VMR9 returns a 32-bit BI_RGB DIB with no color table.
                let header_size = u32::from_le_bytes(bytes[0..4].try_into()?);
                bmp.extend_from_slice(&(14 + header_size).to_le_bytes());
                bmp.extend_from_slice(bytes);
                std::fs::write(path, bmp)?;
                Ok(())
            })();
            self.control.Run()?;
            capture
        }
    }
    pub fn drain_error(&mut self) -> Option<String> {
        // SAFETY: event parameters are freed once, after reading, on the graph's STA thread.
        unsafe {
            let events: IMediaEvent = self.graph.cast().ok()?;
            let (mut code, mut p1, mut p2) = (0, 0, 0);
            while events.GetEvent(&mut code, &mut p1, &mut p2, 0).is_ok() {
                let _ = events.FreeEventParams(code, p1, p2);
                if code == EC_ERRORABORT as i32 {
                    return Some(format!("Capture stopped (HRESULT 0x{:08X}).", p1 as u32));
                }
                if code == EC_DEVICE_LOST as i32 && p2 == 0 {
                    return Some(
                        "Boîtier déconnecté. Rebranchez-le, puis relancez CaptureView.".into(),
                    );
                }
                if code == EC_VIDEO_SIZE_CHANGED as i32 {
                    let (mut w, mut h) = (0, 0);
                    if self.basic.GetVideoSize(&mut w, &mut h).is_ok() && w > 0 && h > 0 {
                        self.width = w;
                        self.height = h;
                        self.attach_resize();
                    }
                }
            }
            None
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        crate::window::set_renderer(None);
        // SAFETY: COM and the owner window remain alive until stop/detach completes.
        unsafe {
            let _ = self.control.Stop();
            let _ = self.video.SetVisible(0);
            let _ = self.video.SetMessageDrain(0);
            let _ = self.video.SetOwner(0);
        }
    }
}

unsafe fn find_device() -> Result<(IBaseFilter, String)> {
    // SAFETY: all COM objects are used locally under initialized STA; no raw pointer escapes.
    unsafe {
        let devices: ICreateDevEnum =
            CoCreateInstance(&CLSID_SystemDeviceEnum, None, CLSCTX_INPROC_SERVER)?;
        let mut enumerator = None;
        devices.CreateClassEnumerator(&CLSID_VideoInputDeviceCategory, &mut enumerator, 0)?;
        let mut found = Vec::new();
        if let Some(enumerator) = enumerator {
            let mut items = [None];
            while enumerator.Next(&mut items, None) == windows::Win32::Foundation::S_OK {
                if let Some(moniker) = items[0].take() {
                    let bag: IPropertyBag = moniker.BindToStorage(None, None)?;
                    let mut value = VARIANT::default();
                    bag.Read(w!("FriendlyName"), &mut value, None)?;
                    let name = BSTR::try_from(&value)?.to_string();
                    if name.to_ascii_lowercase().contains("avermedia") {
                        found.push((moniker, name));
                    }
                }
            }
        }
        if found.len() != 1 {
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                if found.is_empty() {
                    "Aucun boîtier AVerMedia détecté. Vérifiez le câble USB et le pilote."
                } else {
                    "Plusieurs boîtiers AVerMedia détectés. Ne branchez qu'un seul boîtier."
                },
            ));
        }
        let (moniker, name) = found.pop().unwrap();
        Ok((moniker.BindToObject(None, None)?, name))
    }
}
