use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;

/// Available annotation tools
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnnotationTool {
    Rectangle,
    Arrow,
    Pen,
    Highlighter,
    Mosaic,
    Text,
}

/// A single point in a stroke
#[derive(Clone, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// An annotation operation that can be undone/redone
#[derive(Clone, Debug)]
pub struct Annotation {
    pub tool: AnnotationTool,
    pub color: COLORREF,
    pub points: Vec<Point>,
    /// Text content (only for Text tool)
    pub text: Option<String>,
}

/// Annotation engine managing the drawing state and undo/redo stack
pub struct AnnotationEngine {
    /// Completed annotations (the undo stack)
    annotations: Vec<Annotation>,
    /// Redo stack (cleared when a new annotation is added)
    redo_stack: Vec<Annotation>,
    /// Currently in-progress annotation (while mouse is held down)
    current: Option<Annotation>,
    /// Whether we are currently drawing
    drawing: bool,
}

impl AnnotationEngine {
    /// Create a new empty annotation engine
    pub fn new() -> Self {
        Self {
            annotations: Vec::new(),
            redo_stack: Vec::new(),
            current: None,
            drawing: false,
        }
    }

    /// Check if currently drawing
    pub fn is_drawing(&self) -> bool {
        self.drawing
    }

    /// Begin a new stroke at the given position
    pub fn begin_stroke(&mut self, tool: AnnotationTool, color: COLORREF, x: i32, y: i32) {
        self.current = Some(Annotation {
            tool,
            color,
            points: vec![Point { x, y }],
            text: None,
        });
        self.drawing = true;
    }

    /// Continue the current stroke to a new position
    pub fn continue_stroke(&mut self, x: i32, y: i32) {
        if let Some(ref mut annotation) = self.current {
            annotation.points.push(Point { x, y });
        }
    }

    /// End the current stroke
    pub fn end_stroke(&mut self, x: i32, y: i32) {
        if let Some(mut annotation) = self.current.take() {
            annotation.points.push(Point { x, y });
            // Only add if the stroke has meaningful length
            if annotation.points.len() >= 2 {
                self.annotations.push(annotation);
                self.redo_stack.clear(); // New action clears redo
            }
        }
        self.drawing = false;
    }

    /// End the current stroke with associated text (for Text tool)
    pub fn end_stroke_with_text(&mut self, x: i32, y: i32, text: String) {
        if let Some(mut annotation) = self.current.take() {
            annotation.points.push(Point { x, y });
            annotation.text = Some(text);
            if annotation.points.len() >= 2 {
                self.annotations.push(annotation);
                self.redo_stack.clear();
            }
        }
        self.drawing = false;
    }

    /// Undo the last annotation
    pub fn undo(&mut self) {
        if let Some(annotation) = self.annotations.pop() {
            self.redo_stack.push(annotation);
        }
    }

    /// Redo the last undone annotation
    pub fn redo(&mut self) {
        if let Some(annotation) = self.redo_stack.pop() {
            self.annotations.push(annotation);
        }
    }

    /// Render all annotations onto the given HDC
    pub fn render(&self, hdc: HDC) {
        for annotation in &self.annotations {
            render_annotation(hdc, annotation);
        }

        // Also render the in-progress annotation
        if let Some(ref current) = self.current {
            render_annotation(hdc, current);
        }
    }

    /// Composite annotations onto the base image and return final pixel data
    pub fn composite(&self, base_image: &[u8], width: i32, height: i32) -> Vec<u8> {
        unsafe {
            // Create a memory DC and bitmap to draw on
            let hdc_screen = GetDC(None);
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
            let old_bmp = SelectObject(hdc_mem, hbitmap);

            // Draw base image
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // Top-down
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
                base_image.as_ptr() as *const _,
                &bmi,
                DIB_RGB_COLORS,
            );

            // Render all annotations
            self.render(hdc_mem);

            // Extract the final composite pixels
            let mut output = vec![0u8; (width * height * 4) as usize];
            let mut out_bmi = bmi;
            GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                height as u32,
                Some(output.as_mut_ptr() as *mut _),
                &mut out_bmi,
                DIB_RGB_COLORS,
            );

            // Cleanup
            SelectObject(hdc_mem, old_bmp);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);

            output
        }
    }
}

/// Render a single annotation onto the given HDC
fn render_annotation(hdc: HDC, annotation: &Annotation) {
    if annotation.points.is_empty() {
        return;
    }

    unsafe {
        match annotation.tool {
            AnnotationTool::Rectangle => {
                render_rectangle(hdc, annotation);
            }
            AnnotationTool::Arrow => {
                render_arrow(hdc, annotation);
            }
            AnnotationTool::Pen => {
                render_freehand(hdc, annotation, 2, false);
            }
            AnnotationTool::Highlighter => {
                render_freehand(hdc, annotation, 12, true);
            }
            AnnotationTool::Mosaic => {
                render_mosaic(hdc, annotation);
            }
            AnnotationTool::Text => {
                render_text(hdc, annotation);
            }
        }
    }
}

/// Draw a rectangle outline
unsafe fn render_rectangle(hdc: HDC, annotation: &Annotation) {
    if annotation.points.len() < 2 {
        return;
    }

    let start = &annotation.points[0];
    let end = annotation.points.last().unwrap();

    let pen = CreatePen(PS_SOLID, 2, annotation.color);
    let old_pen = SelectObject(hdc, pen);
    let null_brush = GetStockObject(NULL_BRUSH);
    let old_brush = SelectObject(hdc, null_brush);

    Rectangle(hdc, start.x, start.y, end.x, end.y);

    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(pen);
}

/// Draw an arrow from first point to last point
unsafe fn render_arrow(hdc: HDC, annotation: &Annotation) {
    if annotation.points.len() < 2 {
        return;
    }

    let start = &annotation.points[0];
    let end = annotation.points.last().unwrap();

    let pen = CreatePen(PS_SOLID, 2, annotation.color);
    let old_pen = SelectObject(hdc, pen);

    // Draw the line
    MoveToEx(hdc, start.x, start.y, None);
    LineTo(hdc, end.x, end.y);

    // Draw arrowhead
    let dx = (end.x - start.x) as f64;
    let dy = (end.y - start.y) as f64;
    let length = (dx * dx + dy * dy).sqrt();

    if length > 0.0 {
        let arrow_size = 15.0;
        let angle = dy.atan2(dx);
        let arrow_angle = std::f64::consts::PI / 6.0; // 30 degrees

        let x1 = end.x as f64 - arrow_size * (angle - arrow_angle).cos();
        let y1 = end.y as f64 - arrow_size * (angle - arrow_angle).sin();
        let x2 = end.x as f64 - arrow_size * (angle + arrow_angle).cos();
        let y2 = end.y as f64 - arrow_size * (angle + arrow_angle).sin();

        MoveToEx(hdc, end.x, end.y, None);
        LineTo(hdc, x1 as i32, y1 as i32);
        MoveToEx(hdc, end.x, end.y, None);
        LineTo(hdc, x2 as i32, y2 as i32);
    }

    SelectObject(hdc, old_pen);
    let _ = DeleteObject(pen);
}

/// Draw a freehand stroke (pen or highlighter)
unsafe fn render_freehand(hdc: HDC, annotation: &Annotation, width: i32, semi_transparent: bool) {
    if annotation.points.len() < 2 {
        return;
    }

    if semi_transparent {
        // For highlighter, use AlphaBlend with a semi-transparent bitmap.
        // To avoid darkening non-stroke pixels, we first BitBlt the destination
        // region into the temp bitmap, draw the stroke on top, then AlphaBlend
        // back. Non-stroke pixels are identity-blended (dest with itself).
        let min_x = annotation.points.iter().map(|p| p.x).min().unwrap() - width;
        let min_y = annotation.points.iter().map(|p| p.y).min().unwrap() - width;
        let max_x = annotation.points.iter().map(|p| p.x).max().unwrap() + width;
        let max_y = annotation.points.iter().map(|p| p.y).max().unwrap() + width;
        let region_w = max_x - min_x;
        let region_h = max_y - min_y;

        if region_w <= 0 || region_h <= 0 {
            return;
        }

        // Create a temporary DC and bitmap for the highlight stroke
        let hdc_temp = CreateCompatibleDC(hdc);
        let hbm_temp = CreateCompatibleBitmap(hdc, region_w, region_h);
        let old_bmp = SelectObject(hdc_temp, hbm_temp);

        // Copy the destination region into the temp bitmap first.
        // This ensures non-stroke pixels remain identical to the destination
        // after AlphaBlend, preventing any darkening around the stroke.
        let _ = BitBlt(hdc_temp, 0, 0, region_w, region_h, hdc, min_x, min_y, SRCCOPY);

        // Draw the stroke on the temp DC (on top of the copied destination pixels)
        let pen = CreatePen(PS_SOLID, width, annotation.color);
        let old_pen = SelectObject(hdc_temp, pen);

        MoveToEx(hdc_temp, annotation.points[0].x - min_x, annotation.points[0].y - min_y, None);
        for point in &annotation.points[1..] {
            LineTo(hdc_temp, point.x - min_x, point.y - min_y);
        }

        SelectObject(hdc_temp, old_pen);
        let _ = DeleteObject(pen);

        // Alpha blend the highlight onto the main DC.
        // Non-stroke pixels: blend(dest, dest) = dest (identity).
        // Stroke pixels: blend(dest, stroke_color) gives the highlight effect.
        let blend = BLENDFUNCTION {
            BlendOp: 0, // AC_SRC_OVER
            BlendFlags: 0,
            SourceConstantAlpha: 100, // ~40% opacity for highlight effect
            AlphaFormat: 0,
        };

        AlphaBlend(
            hdc,
            min_x,
            min_y,
            region_w,
            region_h,
            hdc_temp,
            0,
            0,
            region_w,
            region_h,
            blend,
        );

        SelectObject(hdc_temp, old_bmp);
        let _ = DeleteObject(hbm_temp);
        let _ = DeleteDC(hdc_temp);
    } else {
        let pen = CreatePen(PS_SOLID, width, annotation.color);
        let old_pen = SelectObject(hdc, pen);

        MoveToEx(hdc, annotation.points[0].x, annotation.points[0].y, None);
        for point in &annotation.points[1..] {
            LineTo(hdc, point.x, point.y);
        }

        SelectObject(hdc, old_pen);
        let _ = DeleteObject(pen);
    }
}

/// Apply mosaic/pixelation effect to the rectangle defined by the annotation
unsafe fn render_mosaic(hdc: HDC, annotation: &Annotation) {
    if annotation.points.len() < 2 {
        return;
    }

    let start = &annotation.points[0];
    let end = annotation.points.last().unwrap();

    let x1 = start.x.min(end.x);
    let y1 = start.y.min(end.y);
    let x2 = start.x.max(end.x);
    let y2 = start.y.max(end.y);

    let region_w = x2 - x1;
    let region_h = y2 - y1;

    if region_w <= 0 || region_h <= 0 {
        return;
    }

    let block_size = 8;

    // Read the entire region into a buffer using GetDIBits for performance
    let hdc_mem = CreateCompatibleDC(hdc);
    let hbm = CreateCompatibleBitmap(hdc, region_w, region_h);
    let old_bmp = SelectObject(hdc_mem, hbm);

    // Copy the region from the source HDC
    let _ = BitBlt(hdc_mem, 0, 0, region_w, region_h, hdc, x1, y1, SRCCOPY);

    // Read pixels into a buffer
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: region_w,
            biHeight: -region_h, // Top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels = vec![0u8; (region_w * region_h * 4) as usize];
    GetDIBits(
        hdc_mem,
        hbm,
        0,
        region_h as u32,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    // Pixelate: for each block, compute average color from the buffer
    let stride = (region_w * 4) as usize;
    let mut y = 0;
    while y < region_h {
        let mut x = 0;
        while x < region_w {
            let bw = block_size.min(region_w - x);
            let bh = block_size.min(region_h - y);

            // Sample the center pixel from the buffer
            let cx = x + bw / 2;
            let cy = y + bh / 2;
            let offset = (cy as usize) * stride + (cx as usize) * 4;
            let b = pixels[offset] as u32;
            let g = pixels[offset + 1] as u32;
            let r = pixels[offset + 2] as u32;
            let color = COLORREF(b | (g << 8) | (r << 16));

            let brush = CreateSolidBrush(color);
            let block_rect = RECT {
                left: x1 + x,
                top: y1 + y,
                right: x1 + x + bw,
                bottom: y1 + y + bh,
            };
            FillRect(hdc, &block_rect, brush);
            let _ = DeleteObject(brush);

            x += block_size;
        }
        y += block_size;
    }

    SelectObject(hdc_mem, old_bmp);
    let _ = DeleteObject(hbm);
    let _ = DeleteDC(hdc_mem);
}

/// Render text annotation at the starting point
unsafe fn render_text(hdc: HDC, annotation: &Annotation) {
    if annotation.points.is_empty() {
        return;
    }

    let text = match annotation.text.as_deref() {
        Some(t) if !t.is_empty() => t,
        _ => return, // No text to render
    };
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

    // Use a larger font for readability
    let font = CreateFontW(
        20, 0, 0, 0,
        400, // FW_NORMAL
        0, 0, 0,
        0, // DEFAULT_CHARSET
        0, 0, 0, 0,
        windows::core::w!("Segoe UI"),
    );
    let old_font = SelectObject(hdc, font);

    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, annotation.color);

    let start = &annotation.points[0];
    TextOutW(
        hdc,
        start.x,
        start.y,
        &text_wide[..text_wide.len() - 1],
    );

    SelectObject(hdc, old_font);
    let _ = DeleteObject(font);
}
