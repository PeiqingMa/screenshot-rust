fn main() {
    // Embed the application icon into the Windows executable.
    // This sets the .exe file icon and provides a resource that can be
    // loaded at runtime for the system tray icon.
    let mut res = winresource::WindowsResource::new();
    res.set_icon("resources/icon.ico");
    // Set resource ID 1 for the icon (standard application icon)
    res.compile().expect("Failed to compile Windows resources");
}
