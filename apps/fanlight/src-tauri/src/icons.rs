//! Icons for the results, as the shell draws them (the same ones Start and File Explorer use), and
//! pictures' thumbnails, turned into small images the page can show. From Lintel.

use base64::Engine;
use windows::core::HSTRING;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{DeleteObject, GetDC, GetDIBits, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ};
use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY, SIIGBF_THUMBNAILONLY};

/// Icons are drawn at up to 32 px in the list, so 64 px covers 200% scaling; the preview's are
/// larger (see `shell::make_icon`).
pub const SIZE_PX: i32 = 64;

/// The icon of a program file, or of an app by its id (`shell:AppsFolder\…`), as a data: URL, at
/// `px` pixels square (or the nearest size it has). The calling thread must have COM initialized.
pub fn icon(parsing_name: &str, px: i32) -> Option<String> {
    data_url(parsing_name, SIIGBF_ICONONLY, px)
}

/// A picture's or video's thumbnail, as File Explorer shows it, as a data: URL.
pub fn thumbnail(path: &str, px: i32) -> Option<String> {
    data_url(path, SIIGBF_THUMBNAILONLY, px)
}

fn data_url(parsing_name: &str, kind: SIIGBF, px: i32) -> Option<String> {
    let (w, h, bgra) = pixels(parsing_name, kind, px)?;
    let bmp = bmp(w, h, &bgra);
    Some(format!("data:image/bmp;base64,{}", base64::engine::general_purpose::STANDARD.encode(bmp)))
}

/// The shell's image of an item: width, height and top-down BGRA pixels with straight alpha.
fn pixels(parsing_name: &str, kind: SIIGBF, px: i32) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let item: IShellItemImageFactory = SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None).ok()?;
        let hbm = item.GetImage(SIZE { cx: px, cy: px }, kind | SIIGBF_BIGGERSIZEOK).ok()?;
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = GetDC(None);
        // First ask the size, then the pixels, as 32-bit top-down rows.
        let ok = GetDIBits(dc, hbm, 0, 0, None, &mut info, DIB_RGB_COLORS) != 0;
        let (w, h) = (info.bmiHeader.biWidth.unsigned_abs(), info.bmiHeader.biHeight.unsigned_abs());
        let mut px = vec![0u8; (w * h * 4) as usize];
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        info.bmiHeader.biHeight = -(h as i32);
        let got = ok && w > 0 && h > 0 && GetDIBits(dc, hbm, 0, h, Some(px.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS) == h as i32;
        ReleaseDC(None, dc);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        got.then(|| {
            unpremultiply(&mut px);
            (w, h, px)
        })
    }
}

/// The shell hands out premultiplied pixels, mostly; an image browsers draw wants straight alpha.
/// If any color is brighter than its alpha, the pixels can't be premultiplied, so they're left be.
fn unpremultiply(px: &mut [u8]) {
    let premultiplied = px.as_chunks::<4>().0.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
    if !premultiplied {
        return;
    }
    for p in px.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// A 32-bit BMP with an alpha channel (a BITMAPV5HEADER with bit masks), which browsers draw
/// with its transparency. Small icons, so the size over PNG doesn't matter.
fn bmp(w: u32, h: u32, bgra: &[u8]) -> Vec<u8> {
    const FILE: u32 = 14;
    const V5: u32 = 124;
    let size = FILE + V5 + bgra.len() as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(FILE + V5).to_le_bytes());
    out.extend_from_slice(&V5.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(-(h as i32)).to_le_bytes()); // top-down
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
    out.extend_from_slice(&(bgra.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 8]); // resolution
    out.extend_from_slice(&[0; 8]); // palette
    for mask in [0x00ff_0000u32, 0x0000_ff00, 0x0000_00ff, 0xff00_0000] {
        out.extend_from_slice(&mask.to_le_bytes());
    }
    out.extend_from_slice(b"BGRs"); // LCS_sRGB, stored little-endian
    out.extend_from_slice(&[0; 36 + 12]); // endpoints, gamma
    out.extend_from_slice(&4u32.to_le_bytes()); // LCS_GM_IMAGES
    out.extend_from_slice(&[0; 12]); // profile, reserved
    out.extend_from_slice(bgra);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_header_is_well_formed() {
        let b = bmp(2, 1, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&b[..2], b"BM");
        assert_eq!(u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize, b.len());
        assert_eq!(u32::from_le_bytes(b[10..14].try_into().unwrap()), 14 + 124);
        assert_eq!(u32::from_le_bytes(b[14..18].try_into().unwrap()), 124);
        assert_eq!(b.len(), 14 + 124 + 8);
        assert_eq!(&b[b.len() - 8..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn unpremultiplies_only_premultiplied_pixels() {
        let mut half = vec![64, 64, 64, 128];
        unpremultiply(&mut half);
        assert_eq!(half, vec![128, 128, 128, 128]);
        // Color brighter than its alpha: straight alpha already.
        let mut straight = vec![200, 10, 10, 128];
        unpremultiply(&mut straight);
        assert_eq!(straight, vec![200, 10, 10, 128]);
    }

    #[test]
    fn draws_a_programs_icon() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let (w, h, px) = pixels(&format!(r"{windir}\explorer.exe"), SIIGBF_ICONONLY, SIZE_PX).expect("explorer's icon");
        assert!(w >= 32 && h >= 32 && px.len() == (w * h * 4) as usize);
        assert!(px.as_chunks::<4>().0.iter().any(|p| p[3] > 0), "not all transparent");
    }
}
