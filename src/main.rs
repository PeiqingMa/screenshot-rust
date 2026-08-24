#![windows_subsystem = "windows"]

mod annotation;
mod capture;
mod clipboard;
mod config;
mod hotkey;
mod overlay;
mod save;
mod toolbar;
mod tray;

use once_cell::sync::Lazy;
use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use config::Config;
use hotkey::HotkeyManager;
use tray::TrayIcon;

/// Global application configuration
static APP_CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::load()));

/// Custom message IDs
pub const WM_TRAY_ICON: u32 = WM_USER + 1;
pub const WM_HOTKEY_CAPTURE: u32 = 1;

fn main() {
    // Load configuration
    let config = APP_CONFIG.lock().unwrap().clone();

    // Create hidden main window (no taskbar presence)
    let hwnd = create_main_window();

    // Initialize system tray icon
    let _tray = TrayIcon::new(hwnd);

    // Register global hotkey
    let _hotkey = HotkeyManager::register(hwnd, &config.hotkey);

    // Run Win32 message loop
    run_message_loop(hwnd);
}

/// Create a hidden main window with WS_EX_TOOLWINDOW to avoid taskbar icon
fn create_main_window() -> HWND {
    unsafe {
        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .expect("Failed to get module handle");

        let class_name = windows::core::w!("RustShotMainClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(main_wnd_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW, // No taskbar icon
            class_name,
            windows::core::w!("RustShot"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create main window");

        hwnd
    }
}

/// Main window procedure handling tray and hotkey messages
unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if wparam.0 == WM_HOTKEY_CAPTURE as usize {
                start_capture(hwnd);
            }
            LRESULT(0)
        }
        WM_TRAY_ICON => {
            tray::handle_tray_message(hwnd, lparam);
            LRESULT(0)
        }
        WM_COMMAND => {
            tray::handle_menu_command(hwnd, wparam);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Start the screen capture process
fn start_capture(_parent: HWND) {
    // Capture all monitors
    if let Some(screenshot) = capture::capture_screen() {
        // Show the overlay for region selection
        overlay::show_overlay(screenshot);
    }
}

/// Run the Win32 message loop
fn run_message_loop(_hwnd: HWND) {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
