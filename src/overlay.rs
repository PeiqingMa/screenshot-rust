use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::capture::ScreenCapture;
use crate::toolbar;

/// Selection rectangle state
#[derive(Clone, Copy, Debug)]
pub struct SelectionRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Resize handle positions
#[derive(Clone, Copy, Debug, PartialEq)]
enum HandlePosition {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    None,
}

/// Interaction mode for the overlay
#[derive(Clone, Copy, Debug, PartialEq)]
enum InteractionMode {
    /// Waiting for user to start drawing (initial state)
    WaitingForDraw,
    /// Drawing a new selection (after first mouse down)
    Drawing,
    /// Moving the selection rectangle
    Moving,
    /// Resizing via a specific handle
    Resizing(HandlePosition),
    /// Selection is complete, waiting for action
    Idle,
}

/// Handle radius in pixels
const HANDLE_RADIUS: i32 = 5;
/// Dim overlay alpha (0-255)
const DIM_ALPHA: u8 = 100;

/// Internal overlay state (stored as window user data)
struct OverlayState {
    capture: ScreenCapture,
    selection: Option<SelectionRect>,
    mode: InteractionMode,
    start_x: i32,
    start_y: i32,
    drag_offset_x: i32,
    drag_offset_y: i32,
    active_handle: HandlePosition,
}

/// Show the fullscreen overlay for region selection
pub fn show_overlay(capture: ScreenCapture) {
    unsafe {
        let instance = GetModuleHandleW(None).expect("Failed to get module handle");

        let class_name = windows::core::w!("RustShotOverlayClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_wnd_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        };

        // Attempt to register the class; if it already exists, that is fine
        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            // Class already registered from a previous capture session - that is OK
            let err = windows::core::Error::from_win32();
            // ERROR_CLASS_ALREADY_EXISTS = 1410
            if err.code().0 as u32 != 0x80070582 {
                // Unexpected error - try to continue anyway
            }
        }

        // Create fullscreen layered window
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            class_name,
            windows::core::w!("RustShot Overlay"),
            WS_POPUP | WS_VISIBLE,
            capture.offset_x,
            capture.offset_y,
            capture.width,
            capture.height,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create overlay window");

        // Set layered window attributes for full opacity (we paint our own alpha)
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).ok();

        // Store overlay state as window user data
        let state = Box::new(OverlayState {
            capture,
            selection: None,
            mode: InteractionMode::WaitingForDraw,
            start_x: 0,
            start_y: 0,
            drag_offset_x: 0,
            drag_offset_y: 0,
            active_handle: HandlePosition::None,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let _ = SetForegroundWindow(hwnd);

        // Local message loop for the overlay
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_QUIT {
                // Re-post WM_QUIT for the main loop (shouldn't happen, but be safe)
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            // Check if our window was destroyed
            if !IsWindow(hwnd).as_bool() {
                break;
            }
        }
    }
}

/// Overlay window procedure
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_overlay(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            handle_mouse_down(hwnd, lparam);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            handle_mouse_move(hwnd, lparam);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            handle_mouse_up(hwnd, lparam);
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            confirm_selection(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u32;
            match key {
                0x0D => {
                    // Enter - confirm selection
                    confirm_selection(hwnd);
                }
                0x1B => {
                    // Escape - cancel
                    cancel_overlay(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // Clean up state
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            // Post a thread message to unblock GetMessageW so the local loop
            // can detect that the window is gone and exit.
            let _ = PostThreadMessageW(GetCurrentThreadId(), WM_NULL, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paint the overlay: dimmed background + selection rectangle + handles + dimensions
unsafe fn paint_overlay(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    if ptr.is_null() {
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let state = &*ptr;

    let width = state.capture.width;
    let height = state.capture.height;

    // Create memory DC with the screenshot
    let hdc_mem = CreateCompatibleDC(hdc);
    let hbitmap = CreateCompatibleBitmap(hdc, width, height);
    let old_bmp = SelectObject(hdc_mem, hbitmap);

    // Draw the captured screen image
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    SetDIBitsToDevice(
        hdc_mem,
        0,
        0,
        width as u32,
        height as u32,
        0,
        0,
        0,
        height as u32,
        state.capture.pixels.as_ptr() as *const _,
        &bmi,
        DIB_RGB_COLORS,
    );

    // Apply dimming overlay to the entire screen
    let dim_brush = CreateSolidBrush(COLORREF(0x00000000));
    let full_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };

    // Draw semi-transparent dim by blending
    // We achieve dimming by drawing a dark rectangle with alpha blending
    let hdc_dim = CreateCompatibleDC(hdc_mem);
    let hbm_dim = CreateCompatibleBitmap(hdc_mem, width, height);
    let old_dim = SelectObject(hdc_dim, hbm_dim);
    let _ = FillRect(hdc_dim, &full_rect, dim_brush);

    // Alpha blend the dim layer onto the screenshot
    let blend = BLENDFUNCTION {
        BlendOp: 0, // AC_SRC_OVER
        BlendFlags: 0,
        SourceConstantAlpha: DIM_ALPHA,
        AlphaFormat: 0,
    };

    let _ = AlphaBlend(
        hdc_mem,
        0,
        0,
        width,
        height,
        hdc_dim,
        0,
        0,
        width,
        height,
        blend,
    );

    let _ = DeleteObject(dim_brush);
    SelectObject(hdc_dim, old_dim);
    let _ = DeleteObject(hbm_dim);
    let _ = DeleteDC(hdc_dim);

    // If there is a selection, draw the clear (undimmed) region
    if let Some(sel) = state.selection {
        if sel.width > 0 && sel.height > 0 {
            // Restore the original screenshot pixels in the selected region by
            // blitting from a clean (undimmed) copy. This avoids the error-prone
            // ySrc calculation that SetDIBitsToDevice requires for partial regions.
            let hdc_clean = CreateCompatibleDC(hdc);
            let hbm_clean = CreateCompatibleBitmap(hdc, width, height);
            let old_clean = SelectObject(hdc_clean, hbm_clean);

            SetDIBitsToDevice(
                hdc_clean,
                0,
                0,
                width as u32,
                height as u32,
                0,
                0,
                0,
                height as u32,
                state.capture.pixels.as_ptr() as *const _,
                &bmi,
                DIB_RGB_COLORS,
            );

            let _ = BitBlt(
                hdc_mem,
                sel.x,
                sel.y,
                sel.width,
                sel.height,
                hdc_clean,
                sel.x,
                sel.y,
                SRCCOPY,
            );

            SelectObject(hdc_clean, old_clean);
            let _ = DeleteObject(hbm_clean);
            let _ = DeleteDC(hdc_clean);

            // Draw blue border around selection
            let blue_pen = CreatePen(PS_SOLID, 2, COLORREF(0x00FF8800)); // Blue in BGR
            let old_pen = SelectObject(hdc_mem, blue_pen);
            let null_brush = GetStockObject(NULL_BRUSH);
            let old_brush_sel = SelectObject(hdc_mem, null_brush);

            let _ = Rectangle(
                hdc_mem,
                sel.x,
                sel.y,
                sel.x + sel.width,
                sel.y + sel.height,
            );

            SelectObject(hdc_mem, old_pen);
            SelectObject(hdc_mem, old_brush_sel);
            let _ = DeleteObject(blue_pen);

            // Draw resize handles (blue circles at corners and edge midpoints)
            draw_handles(hdc_mem, &sel);

            // Draw dimensions label
            draw_dimensions_label(hdc_mem, &sel);
        }
    }

    // Copy from memory DC to screen
    let _ = BitBlt(hdc, 0, 0, width, height, hdc_mem, 0, 0, SRCCOPY);

    SelectObject(hdc_mem, old_bmp);
    let _ = DeleteObject(hbitmap);
    let _ = DeleteDC(hdc_mem);

    let _ = EndPaint(hwnd, &ps);
}

/// Draw resize handles as blue circles
unsafe fn draw_handles(hdc: HDC, sel: &SelectionRect) {
    let blue_brush = CreateSolidBrush(COLORREF(0x00FF8800)); // Blue in BGR
    let old_brush = SelectObject(hdc, blue_brush);
    let null_pen = GetStockObject(NULL_PEN);
    let old_pen = SelectObject(hdc, null_pen);

    let handles = get_handle_positions(sel);
    for (hx, hy) in handles {
        let _ = Ellipse(
            hdc,
            hx - HANDLE_RADIUS,
            hy - HANDLE_RADIUS,
            hx + HANDLE_RADIUS,
            hy + HANDLE_RADIUS,
        );
    }

    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(blue_brush);
}

/// Get the pixel positions of all 8 resize handles
fn get_handle_positions(sel: &SelectionRect) -> Vec<(i32, i32)> {
    let cx = sel.x + sel.width / 2;
    let cy = sel.y + sel.height / 2;
    let r = sel.x + sel.width;
    let b = sel.y + sel.height;

    vec![
        (sel.x, sel.y),     // TopLeft
        (cx, sel.y),        // Top
        (r, sel.y),         // TopRight
        (sel.x, cy),        // Left
        (r, cy),            // Right
        (sel.x, b),         // BottomLeft
        (cx, b),            // Bottom
        (r, b),             // BottomRight
    ]
}

/// Draw the dimensions label above the selection
unsafe fn draw_dimensions_label(hdc: HDC, sel: &SelectionRect) {
    let text = format!("{} x {} px", sel.width, sel.height);
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

    // Position the label above the top-left corner
    let label_x = sel.x;
    let label_y = sel.y - 22;

    // Draw background for the label
    let bg_brush = CreateSolidBrush(COLORREF(0x00FF8800)); // Blue in BGR
    let label_rect = RECT {
        left: label_x,
        top: label_y,
        right: label_x + (text.len() as i32) * 8 + 12,
        bottom: label_y + 20,
    };
    let _ = FillRect(hdc, &label_rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    // Draw text
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, COLORREF(0x00FFFFFF)); // White text
    let mut rc = label_rect;
    DrawTextW(
        hdc,
        &mut text_wide[..text_wide.len() - 1].to_vec(),
        &mut rc,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

/// Handle mouse button down
unsafe fn handle_mouse_down(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

    // Capture mouse so we receive WM_MOUSEMOVE/WM_LBUTTONUP even if cursor
    // leaves the window during the drag operation.
    SetCapture(hwnd);

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    if let Some(sel) = state.selection {
        // Check if clicking on a handle
        let handle = hit_test_handle(&sel, x, y);
        if handle != HandlePosition::None {
            state.mode = InteractionMode::Resizing(handle);
            state.active_handle = handle;
            state.start_x = x;
            state.start_y = y;
            return;
        }

        // Check if clicking inside the selection (move)
        if x >= sel.x && x <= sel.x + sel.width && y >= sel.y && y <= sel.y + sel.height {
            state.mode = InteractionMode::Moving;
            state.drag_offset_x = x - sel.x;
            state.drag_offset_y = y - sel.y;
            return;
        }
    }

    // Start a new selection
    state.mode = InteractionMode::Drawing;
    state.start_x = x;
    state.start_y = y;
    state.selection = Some(SelectionRect {
        x,
        y,
        width: 0,
        height: 0,
    });
}

/// Handle mouse move
unsafe fn handle_mouse_move(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    match state.mode {
        InteractionMode::WaitingForDraw => {
            // Do nothing - waiting for user to click
        }
        InteractionMode::Drawing => {
            // Update selection rectangle from start point to current mouse
            let sx = state.start_x.min(x);
            let sy = state.start_y.min(y);
            let w = (x - state.start_x).abs();
            let h = (y - state.start_y).abs();
            state.selection = Some(SelectionRect {
                x: sx,
                y: sy,
                width: w,
                height: h,
            });
            invalidate_window(hwnd);
        }
        InteractionMode::Moving => {
            if let Some(ref mut sel) = state.selection {
                sel.x = x - state.drag_offset_x;
                sel.y = y - state.drag_offset_y;
                invalidate_window(hwnd);
            }
        }
        InteractionMode::Resizing(handle) => {
            if let Some(ref mut sel) = state.selection {
                apply_resize(sel, handle, x, y);
                invalidate_window(hwnd);
            }
        }
        InteractionMode::Idle => {
            // Update cursor based on hover position
            if let Some(sel) = state.selection {
                let handle = hit_test_handle(&sel, x, y);
                update_cursor(handle, &sel, x, y);
            }
        }
    }
}

/// Handle mouse button up
unsafe fn handle_mouse_up(hwnd: HWND, _lparam: LPARAM) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    match state.mode {
        InteractionMode::Drawing => {
            // First selection completed - if large enough, show the toolbar immediately
            if let Some(sel) = state.selection {
                if sel.width > 5 && sel.height > 5 {
                    let _ = ReleaseCapture();
                    // confirm_selection will destroy the overlay and show toolbar
                    confirm_selection(hwnd);
                    return;
                }
            }
            state.mode = InteractionMode::Idle;
            let _ = ReleaseCapture();
            invalidate_window(hwnd);
        }
        InteractionMode::Moving | InteractionMode::Resizing(_) => {
            state.mode = InteractionMode::Idle;
            let _ = ReleaseCapture();
            invalidate_window(hwnd);
        }
        InteractionMode::WaitingForDraw | InteractionMode::Idle => {}
    }
}

/// Confirm the selection and open the toolbar
unsafe fn confirm_selection(hwnd: HWND) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    if ptr.is_null() {
        return;
    }
    let state = &*ptr;

    if let Some(sel) = state.selection {
        if sel.width > 5 && sel.height > 5 {
            // Crop the selected region
            let cropped = crate::capture::crop_region(&state.capture, sel.x, sel.y, sel.width, sel.height);

            // Calculate the screen position of the selection
            let screen_x = state.capture.offset_x + sel.x;
            let screen_y = state.capture.offset_y + sel.y;

            // Destroy the overlay
            DestroyWindow(hwnd).ok();

            // Show the toolbar for annotation at the selection's screen position
            toolbar::show_toolbar(cropped, sel.width, sel.height, screen_x, screen_y);
        }
    }
}

/// Cancel the overlay without capturing
unsafe fn cancel_overlay(hwnd: HWND) {
    DestroyWindow(hwnd).ok();
}

/// Invalidate the entire overlay window to trigger repaint
unsafe fn invalidate_window(hwnd: HWND) {
    let _ = InvalidateRect(hwnd, None, BOOL::from(false));
}

/// Hit-test which handle (if any) the mouse is over
fn hit_test_handle(sel: &SelectionRect, mx: i32, my: i32) -> HandlePosition {
    let handles = get_handle_positions(sel);
    let handle_types = [
        HandlePosition::TopLeft,
        HandlePosition::Top,
        HandlePosition::TopRight,
        HandlePosition::Left,
        HandlePosition::Right,
        HandlePosition::BottomLeft,
        HandlePosition::Bottom,
        HandlePosition::BottomRight,
    ];

    for (i, (hx, hy)) in handles.iter().enumerate() {
        let dx = mx - hx;
        let dy = my - hy;
        if dx * dx + dy * dy <= (HANDLE_RADIUS + 3) * (HANDLE_RADIUS + 3) {
            return handle_types[i];
        }
    }

    HandlePosition::None
}

/// Apply resize operation based on which handle is being dragged
fn apply_resize(sel: &mut SelectionRect, handle: HandlePosition, mx: i32, my: i32) {
    match handle {
        HandlePosition::TopLeft => {
            let right = sel.x + sel.width;
            let bottom = sel.y + sel.height;
            sel.x = mx;
            sel.y = my;
            sel.width = right - mx;
            sel.height = bottom - my;
        }
        HandlePosition::Top => {
            let bottom = sel.y + sel.height;
            sel.y = my;
            sel.height = bottom - my;
        }
        HandlePosition::TopRight => {
            let bottom = sel.y + sel.height;
            sel.width = mx - sel.x;
            sel.y = my;
            sel.height = bottom - my;
        }
        HandlePosition::Left => {
            let right = sel.x + sel.width;
            sel.x = mx;
            sel.width = right - mx;
        }
        HandlePosition::Right => {
            sel.width = mx - sel.x;
        }
        HandlePosition::BottomLeft => {
            let right = sel.x + sel.width;
            sel.x = mx;
            sel.width = right - mx;
            sel.height = my - sel.y;
        }
        HandlePosition::Bottom => {
            sel.height = my - sel.y;
        }
        HandlePosition::BottomRight => {
            sel.width = mx - sel.x;
            sel.height = my - sel.y;
        }
        HandlePosition::None => {}
    }

    // Ensure minimum size
    if sel.width < 1 {
        sel.width = 1;
    }
    if sel.height < 1 {
        sel.height = 1;
    }
}

/// Update the cursor based on hover position
fn update_cursor(handle: HandlePosition, sel: &SelectionRect, mx: i32, my: i32) {
    unsafe {
        let cursor_id = match handle {
            HandlePosition::TopLeft | HandlePosition::BottomRight => IDC_SIZENWSE,
            HandlePosition::TopRight | HandlePosition::BottomLeft => IDC_SIZENESW,
            HandlePosition::Top | HandlePosition::Bottom => IDC_SIZENS,
            HandlePosition::Left | HandlePosition::Right => IDC_SIZEWE,
            HandlePosition::None => {
                if mx >= sel.x && mx <= sel.x + sel.width && my >= sel.y && my <= sel.y + sel.height
                {
                    IDC_SIZEALL
                } else {
                    IDC_CROSS
                }
            }
        };
        if let Ok(cursor) = LoadCursorW(None, cursor_id) {
            SetCursor(cursor);
        }
    }
}
