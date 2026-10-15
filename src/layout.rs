//! Aspect-preserving placement, independent of Win32 and capture hardware.
pub fn fit(width: i32, height: i32, video_width: i32, video_height: i32) -> (i32, i32, i32, i32) {
    if width <= 0 || height <= 0 || video_width <= 0 || video_height <= 0 {
        return (0, 0, 0, 0);
    }
    let scale = (width as f64 / video_width as f64).min(height as f64 / video_height as f64);
    let w = (video_width as f64 * scale).round() as i32;
    let h = (video_height as f64 * scale).round() as i32;
    ((width - w) / 2, (height - h) / 2, w, h)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_window_keeps_full_picture() {
        assert_eq!(fit(320, 180, 1920, 1080), (0, 0, 320, 180));
    }
    #[test]
    fn four_three_content_has_side_bars() {
        assert_eq!(fit(1920, 1080, 640, 480), (240, 0, 1440, 1080));
    }
    #[test]
    fn portrait_window_does_not_crop() {
        assert_eq!(fit(300, 800, 1920, 1080), (0, 315, 300, 169));
    }
    #[test]
    fn invalid_dimensions_have_no_video_rect() {
        for (w, h, vw, vh) in [
            (0, 600, 1920, 1080),
            (800, 0, 1920, 1080),
            (800, 600, 0, 1080),
            (-1, 600, 1920, 1080),
            (800, 600, 1920, 0),
        ] {
            assert_eq!(fit(w, h, vw, vh), (0, 0, 0, 0));
        }
    }
    #[test]
    fn letterboxes_four_three_window() {
        assert_eq!(fit(800, 600, 1920, 1080), (0, 75, 800, 450));
    }
}
