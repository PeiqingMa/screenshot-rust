use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::CF_DIB;

/// Copy an image (BGRA pixel data) to the Windows clipboard as a DIB
pub fn copy_to_clipboard(pixels: &[u8], width: i32, height: i32) {
    unsafe {
        // Open clipboard
        if !OpenClipboard(None).as_bool() {
            eprintln!("Failed to open clipboard");
            return;
        }

        // Empty existing clipboard content
        if !EmptyClipboard().as_bool() {
            CloseClipboard().ok();
            eprintln!("Failed to empty clipboard");
            return;
        }

        // Create a DIB (Device-Independent Bitmap) header
        let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
        let pixel_data_size = (width * height * 4) as usize;
        let total_size = header_size + pixel_data_size;

        // Allocate global memory for the DIB
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, total_size);
        if hglobal.is_err() {
            CloseClipboard().ok();
            eprintln!("Failed to allocate global memory");
            return;
        }
        let hglobal = hglobal.unwrap();

        let ptr = GlobalLock(hglobal) as *mut u8;
        if ptr.is_null() {
            GlobalFree(hglobal).ok();
            CloseClipboard().ok();
            eprintln!("Failed to lock global memory");
            return;
        }

        // Write BITMAPINFOHEADER
        let header = BITMAPINFOHEADER {
            biSize: header_size as u32,
            biWidth: width,
            biHeight: -height, // Top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: pixel_data_size as u32,
            ..Default::default()
        };

        std::ptr::copy_nonoverlapping(
            &header as *const BITMAPINFOHEADER as *const u8,
            ptr,
            header_size,
        );

        // Write pixel data
        std::ptr::copy_nonoverlapping(
            pixels.as_ptr(),
            ptr.add(header_size),
            pixel_data_size,
        );

        GlobalUnlock(hglobal);

        // Set clipboard data as CF_DIB
        let result = SetClipboardData(CF_DIB.0 as u32, HANDLE(hglobal.0));
        if result.is_err() {
            GlobalFree(hglobal).ok();
            eprintln!("Failed to set clipboard data");
        }

        CloseClipboard().ok();
    }
}
