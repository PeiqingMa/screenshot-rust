use windows::Win32::Foundation::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::WM_TRAY_ICON;

/// Menu command IDs
const ID_MENU_CAPTURE: u32 = 1001;
const ID_MENU_SETTINGS: u32 = 1002;
const ID_MENU_ABOUT: u32 = 1003;
const ID_MENU_EXIT: u32 = 1004;

/// System tray icon manager
pub struct TrayIcon {
    #[allow(dead_code)]
    hwnd: HWND,
    nid: NOTIFYICONDATAW,
}

impl TrayIcon {
    /// Create and show a system tray icon
    pub fn new(hwnd: HWND) -> Self {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            ..Default::default()
        };

        // Set tooltip text
        let tip = "RustShot - Screenshot Tool";
        let tip_wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
        let len = tip_wide.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        // Load default application icon
        unsafe {
            nid.hIcon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }

        Self { hwnd, nid }
    }

    /// Remove the tray icon
    fn remove(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove();
    }
}

/// Handle tray icon messages (right-click context menu, double-click capture)
pub fn handle_tray_message(hwnd: HWND, lparam: LPARAM) {
    let message = (lparam.0 & 0xFFFF) as u32;
    match message {
        WM_RBUTTONUP => {
            show_context_menu(hwnd);
        }
        WM_LBUTTONDBLCLK => {
            // Double-click triggers capture
            crate::start_capture(hwnd);
        }
        _ => {}
    }
}

/// Show the tray context menu
fn show_context_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().expect("Failed to create popup menu");

        // Add menu items
        let _ = AppendMenuW(menu, MF_STRING, ID_MENU_CAPTURE as usize, windows::core::w!("Capture\tCtrl+Shift+S"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_MENU_SETTINGS as usize, windows::core::w!("Settings..."));
        let _ = AppendMenuW(menu, MF_STRING, ID_MENU_ABOUT as usize, windows::core::w!("About RustShot"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_MENU_EXIT as usize, windows::core::w!("Exit"));

        // Get cursor position for menu placement
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);

        // Required to make menu disappear when clicking outside
        let _ = SetForegroundWindow(hwnd);

        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        );

        // Clean up
        let _ = DestroyMenu(menu);
    }
}

/// Handle menu command selections
pub fn handle_menu_command(hwnd: HWND, wparam: WPARAM) {
    let cmd = (wparam.0 & 0xFFFF) as u32;
    match cmd {
        ID_MENU_CAPTURE => {
            crate::start_capture(hwnd);
        }
        ID_MENU_SETTINGS => {
            show_settings_dialog(hwnd);
        }
        ID_MENU_ABOUT => {
            show_about_dialog(hwnd);
        }
        ID_MENU_EXIT => {
            unsafe {
                DestroyWindow(hwnd).ok();
            }
        }
        _ => {}
    }
}

/// Show settings dialog for hotkey configuration
fn show_settings_dialog(_hwnd: HWND) {
    // In a full implementation, this would show a dialog for configuring the hotkey.
    // For now, we show a simple message box.
    unsafe {
        MessageBoxW(
            None,
            windows::core::w!("Hotkey configuration:\n\nEdit rustshot_config.json to change the hotkey binding.\nDefault: Ctrl+Shift+S"),
            windows::core::w!("RustShot Settings"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Show about dialog
fn show_about_dialog(_hwnd: HWND) {
    unsafe {
        MessageBoxW(
            None,
            windows::core::w!("RustShot v0.1.0\n\nA lightweight screenshot tool for Windows.\nPress Ctrl+Shift+S to capture a region.\n\nBuilt with Rust and Win32 API."),
            windows::core::w!("About RustShot"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
