use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT,
};

use crate::config::HotkeyBinding;
use crate::WM_HOTKEY_CAPTURE;

/// Manages a registered global hotkey
pub struct HotkeyManager {
    hwnd: HWND,
    id: u32,
}

impl HotkeyManager {
    /// Register a global hotkey based on the configuration
    pub fn register(hwnd: HWND, binding: &HotkeyBinding) -> Self {
        let mut modifiers = MOD_NOREPEAT;
        if binding.ctrl {
            modifiers |= MOD_CONTROL;
        }
        if binding.shift {
            modifiers |= MOD_SHIFT;
        }
        if binding.alt {
            modifiers |= MOD_ALT;
        }

        let id = WM_HOTKEY_CAPTURE;

        unsafe {
            let result = RegisterHotKey(
                Some(hwnd),
                id as i32,
                modifiers,
                binding.key as u32,
            );
            if !result.as_bool() {
                eprintln!(
                    "Failed to register hotkey: {}. It may be in use by another application.",
                    binding.description
                );
            }
        }

        Self { hwnd, id }
    }

    /// Unregister the hotkey and re-register with a new binding
    pub fn update(&mut self, binding: &HotkeyBinding) {
        self.unregister();

        let mut modifiers = MOD_NOREPEAT;
        if binding.ctrl {
            modifiers |= MOD_CONTROL;
        }
        if binding.shift {
            modifiers |= MOD_SHIFT;
        }
        if binding.alt {
            modifiers |= MOD_ALT;
        }

        unsafe {
            let result = RegisterHotKey(
                Some(self.hwnd),
                self.id as i32,
                modifiers,
                binding.key as u32,
            );
            if !result.as_bool() {
                eprintln!(
                    "Failed to register hotkey: {}",
                    binding.description
                );
            }
        }
    }

    /// Unregister the global hotkey
    fn unregister(&self) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), self.id as i32);
        }
    }

    /// Convert a HOT_KEY_MODIFIERS value to human-readable string
    pub fn modifiers_to_string(modifiers: HOT_KEY_MODIFIERS) -> String {
        let mut parts = Vec::new();
        if modifiers.contains(MOD_CONTROL) {
            parts.push("Ctrl");
        }
        if modifiers.contains(MOD_SHIFT) {
            parts.push("Shift");
        }
        if modifiers.contains(MOD_ALT) {
            parts.push("Alt");
        }
        parts.join("+")
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.unregister();
    }
}
