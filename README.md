# RustShot

A lightweight, fast Windows screenshot tool built with Rust and the Win32 API.

## Features

- **Hotkey-triggered capture**: Press a configurable hotkey (default: `Ctrl+Shift+S`) to start capturing
- **Region selection**: Draw a rectangle on screen to select the capture area
  - Blue border with dimension label (width x height in pixels)
  - Resize handles at corners and edges (blue circles)
  - Drag to move the selection, drag handles to resize
- **Annotation toolbar**: After selecting a region, a floating toolbar appears with:
  - Rectangle drawing tool
  - Arrow tool
  - Freehand pen
  - Highlighter (semi-transparent stroke)
  - Mosaic/pixelate tool (blur sensitive information)
  - Text tool
  - Undo/Redo support
  - Close/Cancel
  - Pin to screen (always on top)
  - Save to file (PNG/JPEG)
  - Copy to clipboard
  - Color picker (cycle through common colors)
- **System tray**: Runs silently in the background with a tray icon
  - No taskbar icon (uses WS_EX_TOOLWINDOW)
  - Right-click context menu: Capture, Settings, About, Exit
  - Double-click tray icon to capture
- **Configuration**: Hotkey and preferences saved to JSON file

## Requirements

- **Operating System**: Windows 10 or later
- **Build Tools**: 
  - [Rust](https://rustup.rs/) (stable toolchain)
  - MSVC build tools (Visual Studio Build Tools or full Visual Studio)

## Building

```bash
# Clone the repository
git clone https://github.com/yourusername/rustshot.git
cd rustshot

# Build in release mode
cargo build --release

# The executable will be at target/release/rustshot.exe
```

## Usage

1. Run `rustshot.exe` - the application starts minimized to the system tray
2. Press `Ctrl+Shift+S` (or your configured hotkey) to start capturing
3. Click and drag to select a region on screen
4. Use the toolbar to annotate, then save or copy the result

### Hotkey Configuration

Edit `rustshot_config.json` (located next to the executable) to change the hotkey:

```json
{
  "hotkey": {
    "ctrl": true,
    "shift": true,
    "alt": false,
    "key": 83,
    "description": "Ctrl+Shift+S"
  },
  "save_directory": null,
  "save_format": "png",
  "auto_copy": false
}
```

The `key` field uses Windows virtual key codes. Common values:
- `83` = S
- `80` = P
- `65` = A
- `49`-`57` = 1-9

### System Tray

Right-click the tray icon for options:
- **Capture**: Start a new capture (same as hotkey)
- **Settings**: View hotkey configuration
- **About**: Show version information
- **Exit**: Close the application

## Architecture

The application is organized into self-contained modules:

| Module | Purpose |
|--------|---------|
| `main.rs` | Entry point, message loop, window creation |
| `config.rs` | Configuration loading/saving (JSON) |
| `hotkey.rs` | Global hotkey registration (RegisterHotKey) |
| `tray.rs` | System tray icon and context menu |
| `capture.rs` | Screen capture via BitBlt (multi-monitor) |
| `overlay.rs` | Fullscreen overlay for region selection |
| `toolbar.rs` | Annotation toolbar UI |
| `annotation.rs` | Drawing tools and undo/redo stack |
| `clipboard.rs` | Copy to clipboard (CF_DIB format) |
| `save.rs` | Save dialog and file writing |

## Dependencies

- [windows-rs](https://github.com/microsoft/windows-rs) - Official Windows API bindings for Rust
- [image](https://crates.io/crates/image) - Image encoding (PNG/JPEG)
- [serde](https://crates.io/crates/serde) / [serde_json](https://crates.io/crates/serde_json) - Configuration serialization
- [arboard](https://crates.io/crates/arboard) - Cross-platform clipboard (fallback)
- [once_cell](https://crates.io/crates/once_cell) - Lazy static initialization

## License

MIT
