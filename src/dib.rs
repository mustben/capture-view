//! Conservative BITMAPINFOHEADER/BI_RGB diagnostic conversion (not playback).
pub fn to_bmp(dib: &[u8]) -> Result<Vec<u8>, &'static str> {
    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }
    if dib.len() < 40 {
        return Err("Truncated DIB header");
    }
    // Deliberately reject BITFIELDS, extended headers and compressed formats.
    if u32_at(dib, 0) != 40 || u32_at(dib, 16) != 0 {
        return Err("Unsupported DIB: require 40-byte BITMAPINFOHEADER and BI_RGB");
    }
    let width = u32_at(dib, 4) as i32;
    let height = u32_at(dib, 8) as i32;
    let planes = u16::from_le_bytes(dib[12..14].try_into().unwrap());
    let bits = u16::from_le_bytes(dib[14..16].try_into().unwrap());
    if width <= 0 || height == 0 || height == i32::MIN || planes != 1 {
        return Err("Invalid DIB dimensions or planes");
    }
    if ![1, 4, 8, 24, 32].contains(&bits) {
        return Err("Unsupported BI_RGB bit depth");
    }
    let used = u32_at(dib, 32);
    let colors = if bits <= 8 {
        if used > (1 << bits) {
            return Err("Invalid DIB palette size");
        }
        if used == 0 { 1 << bits } else { used }
    } else {
        used
    };
    let offset = 40u32
        .checked_add(colors.checked_mul(4).ok_or("Palette overflow")?)
        .ok_or("DIB offset overflow")?;
    let stride = (width as u32)
        .checked_mul(bits as u32)
        .and_then(|n| n.checked_add(31))
        .map(|n| (n / 32) * 4)
        .ok_or("DIB stride overflow")?;
    let pixels = stride
        .checked_mul(height.unsigned_abs())
        .ok_or("DIB image overflow")?;
    let end = offset.checked_add(pixels).ok_or("DIB length overflow")?;
    let declared = u32_at(dib, 20);
    if end as usize > dib.len() || (declared != 0 && declared != pixels) {
        return Err("Truncated pixels or inconsistent DIB image size");
    }
    let size = u32::try_from(dib.len())
        .ok()
        .and_then(|n| n.checked_add(14))
        .ok_or("BMP file size overflow")?;
    let pixel_offset = offset.checked_add(14).ok_or("BMP offset overflow")?;
    let mut bmp = Vec::with_capacity(size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Ok(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dib(bits: u16, palette: usize) -> Vec<u8> {
        let mut d = vec![0; 40 + palette * 4 + 4];
        d[..4].copy_from_slice(&40u32.to_le_bytes());
        d[4..8].copy_from_slice(&1i32.to_le_bytes());
        d[8..12].copy_from_slice(&(-1i32).to_le_bytes());
        d[12..14].copy_from_slice(&1u16.to_le_bytes());
        d[14..16].copy_from_slice(&bits.to_le_bytes());
        *d.last_mut().unwrap() = 123;
        d
    }
    #[test]
    fn rgb_and_palette_offsets() {
        for (bits, palette) in [(32, 0), (24, 0), (8, 256), (4, 16), (1, 2)] {
            let d = dib(bits, palette);
            let b = to_bmp(&d).unwrap();
            assert_eq!(
                u32::from_le_bytes(b[10..14].try_into().unwrap()),
                54 + palette as u32 * 4
            );
            assert_eq!(
                u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize,
                b.len()
            );
            assert_eq!(&b[14..], &d);
            assert_eq!(*b.last().unwrap(), 123);
        }
    }
    #[test]
    fn rejects_malformed_and_unsupported() {
        assert!(to_bmp(&[0; 39]).is_err());
        for (at, value) in [
            (0, 108),
            (4, 0),
            (8, 0x80000000),
            (16, 3),
            (20, 99),
            (32, u32::MAX),
        ] {
            let mut d = dib(32, 0);
            d[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(to_bmp(&d).is_err(), "field {at}");
        }
        let mut d = dib(32, 0);
        d[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(to_bmp(&d).is_err());
        assert!(to_bmp(&dib(16, 0)).is_err());
        assert!(to_bmp(&dib(8, 0)).is_err());
    }
}
