use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::annotation::{AnnotationEngine, AnnotationTool};

/// Toolbar button IDs
const BTN_RECTANGLE: u32 = 2001;
const BTN_ARROW: u32 = 2002;
const BTN_PEN: u32 = 2003;
const BTN_HIGHLIGHTER: u32 = 2004;
const BTN_MOSAIC: u32 = 2005;
const BTN_TEXT: u32 = 2006;
const BTN_UNDO: u32 = 2007;
const BTN_REDO: u32 = 2008;
const BTN_CLOSE: u32 = 2009;
const BTN_PIN: u32 = 2010;
const BTN_SAVE: u32 = 2011;
const BTN_COPY: u32 = 2012;
const BTN_COLOR: u32 = 2013;

/// Toolbar button definition
struct ToolbarButton {
    id: u32,
    label: &'static str,
    #[allow(dead_code)]
    tooltip: &'static str,
}

/// All toolbar buttons in order
const BUTTONS: &[ToolbarButton] = &[
    ToolbarButton { id: BTN_RECTANGLE, label: "\u{25A1}", tooltip: "Rectangle" },
    ToolbarButton { id: BTN_ARROW, label: "\u{2197}", tooltip: "Arrow" },
    ToolbarButton { id: BTN_PEN, label: "\u{270E}", tooltip: "Pen" },
    ToolbarButton { id: BTN_HIGHLIGHTER, label: "\u{2593}", tooltip: "Highlighter" },
    ToolbarButton { id: BTN_MOSAIC, label: "\u{25A6}", tooltip: "Mosaic" },
    ToolbarButton { id: BTN_TEXT, label: "T", tooltip: "Text" },
    ToolbarButton { id: BTN_UNDO, label: "\u{21B6}", tooltip: "Undo" },
    ToolbarButton { id: BTN_REDO, label: "\u{21B7}", tooltip: "Redo" },
    ToolbarButton { id: BTN_CLOSE, label: "\u{2715}", tooltip: "Close" },
    ToolbarButton { id: BTN_PIN, label: "\u{2299}", tooltip: "Pin to Screen" },
    ToolbarButton { id: BTN_SAVE, label: "\u{21E9}", tooltip: "Save" },
    ToolbarButton { id: BTN_COPY, label: "\u{29C9}", tooltip: "Copy" },
    ToolbarButton { id: BTN_COLOR, label: "\u{25CF}", tooltip: "Color Picker" },
];

/// Toolbar button dimensions
const BTN_WIDTH: i32 = 32;
const BTN_HEIGHT: i32 = 32;
const TOOLBAR_PADDING: i32 = 4;
const TOOLBAR_HEIGHT: i32 = BTN_HEIGHT + TOOLBAR_PADDING * 2;

/// Toolbar state
struct ToolbarState {
    image_data: Vec<u8>,
    image_width: i32,
    image_height: i32,
    annotation_engine: AnnotationEngine,
    current_tool: AnnotationTool,
    current_color: COLORREF,
    pinned: bool,
    canvas_hwnd: HWND,
    #[allow(dead_code)]
    toolbar_hwnd: HWND,
}

/// Show the annotation toolbar and canvas for the captured region
pub fn show_toolbar(image_data: Vec<u8>, width: i32, height: i32, screen_x: i32, screen_y: i32) {
    unsafe {
        let instance = GetModuleHandleW(None).expect("Failed to get module handle");

        // Register toolbar window class
        let toolbar_class = windows::core::w!("RustShotToolbarClass");
        let canvas_class = windows::core::w!("RustShotCanvasClass");

        let wc_toolbar = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(toolbar_wnd_proc),
            hInstance: instance.into(),
            lpszClassName: toolbar_class,
            hbrBackground: CreateSolidBrush(COLORREF(0x00383838)),
            ..Default::default()
        };

        let wc_canvas = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(canvas_wnd_proc),
            hInstance: instance.into(),
            lpszClassName: canvas_class,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        };

        // Attempt registration; ignore if class already exists
        RegisterClassExW(&wc_toolbar);
        RegisterClassExW(&wc_canvas);

        // Position the canvas at the exact screen location where the selection was made
        let canvas_x = screen_x;
        let canvas_y = screen_y;

        // Create canvas window (shows the captured image + annotations)
        let canvas_hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            canvas_class,
            windows::core::w!("RustShot Canvas"),
            WS_POPUP | WS_VISIBLE | WS_BORDER,
            canvas_x,
            canvas_y,
            width,
            height,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create canvas window");

        // Create toolbar window below the canvas
        let toolbar_width = BUTTONS.len() as i32 * BTN_WIDTH + TOOLBAR_PADDING * 2;
        let toolbar_x = canvas_x + (width - toolbar_width) / 2;
        let toolbar_y = canvas_y + height + 4;

        let toolbar_hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            toolbar_class,
            windows::core::w!("RustShot Toolbar"),
            WS_POPUP | WS_VISIBLE,
            toolbar_x,
            toolbar_y,
            toolbar_width,
            TOOLBAR_HEIGHT,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create toolbar window");

        // Create shared state - owned solely by this function scope
        let state = Box::new(ToolbarState {
            image_data,
            image_width: width,
            image_height: height,
            annotation_engine: AnnotationEngine::new(),
            current_tool: AnnotationTool::Rectangle,
            current_color: COLORREF(0x000000FF), // Red in BGR
            pinned: false,
            canvas_hwnd,
            toolbar_hwnd,
        });
        let state_ptr = Box::into_raw(state);

        // Store state in both windows (neither window owns the allocation)
        SetWindowLongPtrW(toolbar_hwnd, GWLP_USERDATA, state_ptr as isize);
        SetWindowLongPtrW(canvas_hwnd, GWLP_USERDATA, state_ptr as isize);

        let _ = ShowWindow(canvas_hwnd, SW_SHOW);
        let _ = ShowWindow(toolbar_hwnd, SW_SHOW);

        // Message loop for toolbar/canvas windows
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            // Break if both windows are destroyed
            if !IsWindow(toolbar_hwnd).as_bool() && !IsWindow(canvas_hwnd).as_bool() {
                break;
            }
        }

        // Single ownership cleanup - only this scope frees the state
        let _ = Box::from_raw(state_ptr);
    }
}

/// Toolbar window procedure
unsafe extern "system" fn toolbar_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_toolbar(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            handle_toolbar_click(hwnd, lparam);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u32;
            if key == 0x1B {
                // VK_ESCAPE - close toolbar and canvas
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
                if !ptr.is_null() {
                    let state = &*ptr;
                    let canvas = state.canvas_hwnd;
                    DestroyWindow(canvas).ok();
                    DestroyWindow(hwnd).ok();
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // Clear the user data pointer but do NOT free -- show_toolbar owns the allocation
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            // Post a thread message to unblock GetMessageW so the local loop
            // can detect that the window is gone and exit.
            let _ = PostThreadMessageW(GetCurrentThreadId(), WM_NULL, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Canvas window procedure (for drawing annotations)
unsafe extern "system" fn canvas_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_canvas(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            handle_canvas_mouse_down(hwnd, lparam);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            handle_canvas_mouse_move(hwnd, lparam);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            handle_canvas_mouse_up(hwnd, lparam);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u32;
            if key == 0x1B {
                // VK_ESCAPE - close canvas and toolbar
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
                if !ptr.is_null() {
                    let state = &*ptr;
                    let toolbar = state.toolbar_hwnd;
                    DestroyWindow(hwnd).ok();
                    DestroyWindow(toolbar).ok();
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // Clear the user data pointer but do NOT free -- show_toolbar owns the allocation
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paint the toolbar with button icons
unsafe fn paint_toolbar(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;

    // Draw background
    let bg_brush = CreateSolidBrush(COLORREF(0x00383838));
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect).ok();
    FillRect(hdc, &rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    // Draw buttons
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, COLORREF(0x00FFFFFF)); // White text

    // Create a font that properly renders the Unicode symbols
    let font = CreateFontW(
        20, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0,
        windows::core::w!("Segoe UI Symbol"),
    );
    let old_font = SelectObject(hdc, font);

    for (i, button) in BUTTONS.iter().enumerate() {
        let bx = TOOLBAR_PADDING + i as i32 * BTN_WIDTH;
        let by = TOOLBAR_PADDING;

        // Highlight active tool
        if !ptr.is_null() {
            let state = &*ptr;
            let is_active = match button.id {
                BTN_RECTANGLE => state.current_tool == AnnotationTool::Rectangle,
                BTN_ARROW => state.current_tool == AnnotationTool::Arrow,
                BTN_PEN => state.current_tool == AnnotationTool::Pen,
                BTN_HIGHLIGHTER => state.current_tool == AnnotationTool::Highlighter,
                BTN_MOSAIC => state.current_tool == AnnotationTool::Mosaic,
                BTN_TEXT => state.current_tool == AnnotationTool::Text,
                _ => false,
            };

            if is_active {
                let active_brush = CreateSolidBrush(COLORREF(0x00666666));
                let active_rect = RECT {
                    left: bx,
                    top: by,
                    right: bx + BTN_WIDTH,
                    bottom: by + BTN_HEIGHT,
                };
                FillRect(hdc, &active_rect, active_brush);
                let _ = DeleteObject(active_brush);
            }

            // Special color for the color picker button
            if button.id == BTN_COLOR {
                SetTextColor(hdc, state.current_color);
            }
        }

        // Draw button label
        let label_wide: Vec<u16> = button.label.encode_utf16().chain(std::iter::once(0)).collect();
        let mut btn_rect = RECT {
            left: bx,
            top: by,
            right: bx + BTN_WIDTH,
            bottom: by + BTN_HEIGHT,
        };
        DrawTextW(
            hdc,
            &mut label_wide[..label_wide.len() - 1].to_vec(),
            &mut btn_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // Reset text color after color button
        if !ptr.is_null() {
            let state = &*ptr;
            if button.id == BTN_COLOR {
                SetTextColor(hdc, COLORREF(0x00FFFFFF));
            }
            let _ = state;
        }
    }

    SelectObject(hdc, old_font);
    let _ = DeleteObject(font);

    let _ = EndPaint(hwnd, &ps);
}

/// Paint the canvas (captured image + annotations)
unsafe fn paint_canvas(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
    if ptr.is_null() {
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let state = &*ptr;

    // Draw the base captured image
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: state.image_width,
            biHeight: -state.image_height, // Top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    SetDIBitsToDevice(
        hdc,
        0,
        0,
        state.image_width as u32,
        state.image_height as u32,
        0,
        0,
        0,
        state.image_height as u32,
        state.image_data.as_ptr() as *const _,
        &bmi,
        DIB_RGB_COLORS,
    );

    // Draw all annotations on top
    state.annotation_engine.render(hdc);

    let _ = EndPaint(hwnd, &ps);
}

/// Handle toolbar button clicks
unsafe fn handle_toolbar_click(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    // Determine which button was clicked
    let btn_index = (x - TOOLBAR_PADDING) / BTN_WIDTH;
    if btn_index < 0 || btn_index >= BUTTONS.len() as i32 {
        return;
    }

    let button_id = BUTTONS[btn_index as usize].id;

    match button_id {
        BTN_RECTANGLE => state.current_tool = AnnotationTool::Rectangle,
        BTN_ARROW => state.current_tool = AnnotationTool::Arrow,
        BTN_PEN => state.current_tool = AnnotationTool::Pen,
        BTN_HIGHLIGHTER => state.current_tool = AnnotationTool::Highlighter,
        BTN_MOSAIC => state.current_tool = AnnotationTool::Mosaic,
        BTN_TEXT => state.current_tool = AnnotationTool::Text,
        BTN_UNDO => {
            state.annotation_engine.undo();
            let _ = InvalidateRect(state.canvas_hwnd, None, BOOL::from(true));
        }
        BTN_REDO => {
            state.annotation_engine.redo();
            let _ = InvalidateRect(state.canvas_hwnd, None, BOOL::from(true));
        }
        BTN_CLOSE => {
            let canvas = state.canvas_hwnd;
            // Destroy both windows; toolbar's WM_DESTROY will post quit
            DestroyWindow(canvas).ok();
            DestroyWindow(hwnd).ok();
            return;
        }
        BTN_PIN => {
            state.pinned = !state.pinned;
            // Pin removes the toolbar and makes canvas always-on-top
        }
        BTN_SAVE => {
            let final_image = state.annotation_engine.composite(
                &state.image_data,
                state.image_width,
                state.image_height,
            );
            crate::save::save_image(&final_image, state.image_width, state.image_height);
        }
        BTN_COPY => {
            let final_image = state.annotation_engine.composite(
                &state.image_data,
                state.image_width,
                state.image_height,
            );
            crate::clipboard::copy_to_clipboard(&final_image, state.image_width, state.image_height);
        }
        BTN_COLOR => {
            // Cycle through common colors
            state.current_color = match state.current_color.0 {
                0x000000FF => COLORREF(0x0000FF00), // Red -> Green
                0x0000FF00 => COLORREF(0x00FF0000), // Green -> Blue
                0x00FF0000 => COLORREF(0x0000FFFF), // Blue -> Yellow
                0x0000FFFF => COLORREF(0x00FFFFFF), // Yellow -> White
                _ => COLORREF(0x000000FF),          // Default to Red
            };
        }
        _ => {}
    }

    // Repaint toolbar to show active tool
    let _ = InvalidateRect(hwnd, None, BOOL::from(true));
}

/// Handle mouse down on canvas for drawing
unsafe fn handle_canvas_mouse_down(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    state.annotation_engine.begin_stroke(state.current_tool, state.current_color, x, y);
    SetCapture(hwnd);
}

/// Handle mouse move on canvas for drawing
unsafe fn handle_canvas_mouse_move(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    if state.annotation_engine.is_drawing() {
        state.annotation_engine.continue_stroke(x, y);
        let _ = InvalidateRect(hwnd, None, BOOL::from(false));
    }
}

/// Handle mouse up on canvas for drawing
unsafe fn handle_canvas_mouse_up(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ToolbarState;
    if ptr.is_null() {
        return;
    }
    let state = &mut *ptr;

    if state.current_tool == AnnotationTool::Text {
        // Prompt user for text input via a simple input dialog
        let text = prompt_text_input(hwnd);
        if let Some(text) = text {
            if !text.is_empty() {
                state.annotation_engine.end_stroke_with_text(x, y, text);
            } else {
                // Empty text - cancel the annotation
                state.annotation_engine.end_stroke(x, y);
            }
        } else {
            // User cancelled
            state.annotation_engine.end_stroke(x, y);
        }
    } else {
        state.annotation_engine.end_stroke(x, y);
    }

    let _ = ReleaseCapture();
    let _ = InvalidateRect(hwnd, None, BOOL::from(false));
}

/// User-defined message posted by the text dialog's wndproc when WM_COMMAND is received.
/// WPARAM carries the control ID (1 = OK, 2 = Cancel).
const WM_TEXT_DLG_COMMAND: u32 = WM_APP;

/// Custom window procedure for the text input dialog.
/// Handles WM_COMMAND from button clicks (sent synchronously by the button controls)
/// and re-posts them as WM_APP so the modal message loop can see them.
unsafe extern "system" fn text_dlg_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;
            let ctrl_id = (wparam.0 & 0xFFFF) as u32;
            // BN_CLICKED == 0
            if notification == 0 && (ctrl_id == 1 || ctrl_id == 2) {
                // Post a user message so the modal loop picks it up
                let _ = PostMessageW(hwnd, WM_TEXT_DLG_COMMAND, WPARAM(ctrl_id as usize), LPARAM(0));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Prompt the user for text input using a simple dialog with an edit control.
/// Returns Some(text) if confirmed, None if cancelled.
unsafe fn prompt_text_input(parent: HWND) -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::*;

    // Use a simple approach: create a popup window with an edit control
    let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
        .expect("Failed to get module handle");

    let class_name = windows::core::w!("RustShotTextInputClass");

    // Register class with a custom wndproc that handles WM_COMMAND from buttons
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(text_dlg_wnd_proc),
        hInstance: instance.into(),
        lpszClassName: class_name,
        hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
        ..Default::default()
    };
    // Ignore if already registered
    RegisterClassExW(&wc);

    // Create a small dialog window
    let dlg_width = 300;
    let dlg_height = 120;
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let screen_h = GetSystemMetrics(SM_CYSCREEN);
    let dlg_x = (screen_w - dlg_width) / 2;
    let dlg_y = (screen_h - dlg_height) / 2;

    let dlg = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
        class_name,
        windows::core::w!("Enter Text"),
        WS_POPUP | WS_VISIBLE | WS_CAPTION | WS_SYSMENU,
        dlg_x,
        dlg_y,
        dlg_width,
        dlg_height,
        parent,
        None,
        instance,
        None,
    ).ok()?;

    // Create edit control with WS_TABSTOP for keyboard navigation
    let edit = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        windows::core::w!("EDIT"),
        windows::core::w!(""),
        WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | WINDOW_STYLE(0x0080), // ES_AUTOHSCROLL
        10,
        10,
        dlg_width - 20,
        24,
        dlg,
        None,
        instance,
        None,
    ).ok()?;

    // Create OK button with WS_TABSTOP
    let _ok_btn = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        windows::core::w!("BUTTON"),
        windows::core::w!("OK"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x00010000), // BS_DEFPUSHBUTTON
        dlg_width / 2 - 80,
        50,
        70,
        28,
        dlg,
        HMENU(std::ptr::dangling_mut()),
        instance,
        None,
    );

    // Create Cancel button with WS_TABSTOP
    let _cancel_btn = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        windows::core::w!("BUTTON"),
        windows::core::w!("Cancel"),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        dlg_width / 2 + 10,
        50,
        70,
        28,
        dlg,
        HMENU(2isize as *mut _),
        instance,
        None,
    );

    let _ = SetFocus(edit);

    // Run a modal message loop for this dialog
    let mut result: Option<String> = None;
    let mut msg = MSG::default();
    loop {
        let ret = GetMessageW(&mut msg, None, 0, 0);
        if !ret.as_bool() {
            break;
        }

        // Check for our user-defined message posted by the custom wndproc on button click
        if msg.message == WM_TEXT_DLG_COMMAND {
            let ctrl_id = msg.wParam.0 as u32;
            if ctrl_id == 1 {
                // OK pressed
                let mut buf = [0u16; 512];
                let len = GetWindowTextW(edit, &mut buf);
                let text = String::from_utf16_lossy(&buf[..len as usize]);
                result = Some(text);
                break;
            } else if ctrl_id == 2 {
                // Cancel pressed
                result = None;
                break;
            }
        }

        // Handle Enter/Escape keys
        if msg.message == WM_KEYDOWN {
            let key = msg.wParam.0 as u32;
            if key == 0x0D {
                // Enter - confirm
                let mut buf = [0u16; 512];
                let len = GetWindowTextW(edit, &mut buf);
                let text = String::from_utf16_lossy(&buf[..len as usize]);
                result = Some(text);
                break;
            } else if key == 0x1B {
                // Escape - cancel
                result = None;
                break;
            }
        }

        // Check if dialog was closed
        if !IsWindow(dlg).as_bool() {
            result = None;
            break;
        }

        // Use IsDialogMessage for Tab key navigation between controls
        if IsDialogMessageW(dlg, &msg).as_bool() {
            continue;
        }

        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    DestroyWindow(dlg).ok();
    result
}
