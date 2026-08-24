use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Captured screen data including the bitmap pixels and dimensions
#[derive(Clone)]
pub struct ScreenCapture {
    /// Raw BGRA pixel data
    pub pixels: Vec<u8>,
    /// Total width of the captured area
    pub width: i32,
    /// Total height of the captured area
    pub height: i32,
    /// X offset of the virtual screen (can be negative for multi-monitor)
    pub offset_x: i32,
    /// Y offset of the virtual screen (can be negative for multi-monitor)
    pub offset_y: i32,
}

/// Capture the entire virtual screen (all monitors)
pub fn capture_screen() -> Option<ScreenCapture> {
    unsafe {
        // Get virtual screen dimensions (covers all monitors)
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        if width <= 0 || height <= 0 {
            return None;
        }

        // Get the desktop window DC
        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return None;
        }

        // Create a compatible DC and bitmap
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            ReleaseDC(None, hdc_screen);
            return None;
        }

        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return None;
        }

        // Select bitmap into memory DC and perform BitBlt
        let old_bitmap = SelectObject(hdc_mem, hbitmap);
        let success = BitBlt(
            hdc_mem,
            0,
            0,
            width,
            height,
            Some(hdc_screen),
            x,
            y,
            SRCCOPY,
        );

        if !success.as_bool() {
            SelectObject(hdc_mem, old_bitmap);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return None;
        }

        // Extract pixel data from the bitmap
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // Top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let buffer_size = (width * height * 4) as usize;
        let mut pixels = vec![0u8; buffer_size];

        let lines = GetDIBits(
            hdc_mem,
            hbitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Cleanup GDI objects
        SelectObject(hdc_mem, old_bitmap);
        let _ = DeleteObject(hbitmap);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        if lines == 0 {
            return None;
        }

        Some(ScreenCapture {
            pixels,
            width,
            height,
            offset_x: x,
            offset_y: y,
        })
    }
}

/// Crop a region from the screen capture
pub fn crop_region(capture: &ScreenCapture, x: i32, y: i32, w: i32, h: i32) -> Vec<u8> {
    let mut cropped = Vec::with_capacity((w * h * 4) as usize);

    for row in y..(y + h) {
        if row < 0 || row >= capture.height {
            // Fill with black for out-of-bounds rows
            cropped.extend(std::iter::repeat(0u8).take((w * 4) as usize));
            continue;
        }
        for col in x..(x + w) {
            if col < 0 || col >= capture.width {
                cropped.extend_from_slice(&[0, 0, 0, 255]);
            } else {
                let idx = ((row * capture.width + col) * 4) as usize;
                // BGRA format
                cropped.push(capture.pixels[idx]);     // B
                cropped.push(capture.pixels[idx + 1]); // G
                cropped.push(capture.pixels[idx + 2]); // R
                cropped.push(capture.pixels[idx + 3]); // A
            }
        }
    }

    cropped
}
