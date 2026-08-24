use windows::Win32::Foundation::*;
use windows::Win32::UI::Controls::Dialogs::*;

/// Show a Save File dialog and save the image to disk
pub fn save_image(pixels: &[u8], width: i32, height: i32) {
    let file_path = show_save_dialog();
    if let Some(path) = file_path {
        write_image_file(&path, pixels, width, height);
    }
}

/// Show the Windows Save File dialog
fn show_save_dialog() -> Option<String> {
    unsafe {
        // Buffer for the file path
        let mut file_name = vec![0u16; 260];

        let filter = encode_filter("PNG Image (*.png)\0*.png\0JPEG Image (*.jpg)\0*.jpg\0All Files (*.*)\0*.*\0\0");

        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: HWND::default(),
            lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
            lpstrFile: windows::core::PWSTR(file_name.as_mut_ptr()),
            nMaxFile: file_name.len() as u32,
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            lpstrDefExt: windows::core::w!("png"),
            ..Default::default()
        };

        if GetSaveFileNameW(&mut ofn).as_bool() {
            // Find the null terminator
            let len = file_name.iter().position(|&c| c == 0).unwrap_or(file_name.len());
            let path = String::from_utf16_lossy(&file_name[..len]);
            Some(path)
        } else {
            None
        }
    }
}

/// Encode a filter string to wide chars
fn encode_filter(filter: &str) -> Vec<u16> {
    filter.encode_utf16().collect()
}

/// Write the image data to a file using the `image` crate
fn write_image_file(path: &str, pixels: &[u8], width: i32, height: i32) {
    // Convert BGRA to RGBA for the image crate
    let mut rgba_pixels = Vec::with_capacity(pixels.len());
    for chunk in pixels.chunks(4) {
        if chunk.len() == 4 {
            rgba_pixels.push(chunk[2]); // R (from B position in BGRA)
            rgba_pixels.push(chunk[1]); // G
            rgba_pixels.push(chunk[0]); // B (from R position in BGRA)
            rgba_pixels.push(chunk[3]); // A
        }
    }

    // Determine format from extension
    let path_lower = path.to_lowercase();

    let result = if path_lower.ends_with(".jpg") || path_lower.ends_with(".jpeg") {
        // Save as JPEG (convert to RGB since JPEG doesn't support alpha)
        let mut rgb_pixels = Vec::with_capacity((width * height * 3) as usize);
        for chunk in rgba_pixels.chunks(4) {
            if chunk.len() == 4 {
                rgb_pixels.push(chunk[0]); // R
                rgb_pixels.push(chunk[1]); // G
                rgb_pixels.push(chunk[2]); // B
            }
        }
        image::save_buffer(
            path,
            &rgb_pixels,
            width as u32,
            height as u32,
            image::ColorType::Rgb8,
        )
    } else {
        // Default: save as PNG
        image::save_buffer(
            path,
            &rgba_pixels,
            width as u32,
            height as u32,
            image::ColorType::Rgba8,
        )
    };

    if let Err(e) = result {
        eprintln!("Failed to save image: {}", e);
        unsafe {
            let msg = format!("Failed to save image:\n{}\0", e);
            let msg_wide: Vec<u16> = msg.encode_utf16().collect();
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                windows::core::PCWSTR(msg_wide.as_ptr()),
                windows::core::w!("Save Error"),
                windows::Win32::UI::WindowsAndMessaging::MB_OK
                    | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}
