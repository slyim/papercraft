//! Artboard painting shared by the canvas and headless PNG export.
//!
//! PNG export runs the same [`paint_document`] the canvas uses through a windowless
//! egui context, tessellates the shapes with egui's own tessellator and rasterizes the
//! resulting triangles in software. Text therefore wraps and shapes exactly as it does
//! on the canvas, with the bundled egui fonts.

use crate::document::{Document, NodeKind};
use eframe::egui::{
    self, Color32, FontId, LayerId, Painter, Pos2, Rect, TextureId, epaint::Primitive, pos2, vec2,
};
use std::collections::HashMap;

/// Largest PNG side, in pixels.
pub const MAX_PNG_SIDE: u32 = 16_384;
/// Largest PNG area, in pixels (1 GiB for the floating-point working buffer).
pub const MAX_PNG_PIXELS: u64 = 64 * 1024 * 1024;
/// Accepted PNG export scales.
pub const PNG_SCALE: std::ops::RangeInclusive<f32> = 0.1..=8.0;

pub fn color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Fonts used by the editor and by headless rendering.
pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts
}

/// Paints the artboard and its visible layers. `preview` rounds font sizes so small
/// thumbnails do not rasterize a separate glyph set for every scale. The text of
/// `editing` is left to the in-place editor.
pub fn paint_document(
    painter: &Painter,
    document: &Document,
    origin: Pos2,
    zoom: f32,
    clip: Rect,
    preview: bool,
    editing: Option<u64>,
) {
    let artboard = Rect::from_min_size(origin, vec2(document.width, document.height) * zoom);
    let painter = painter.with_clip_rect(clip);
    painter.rect_filled(artboard, 0.0, color(document.background_color()));
    let content = painter.with_clip_rect(clip.intersect(artboard));
    for node in document.nodes.iter().filter(|n| !n.hidden) {
        let rect = Rect::from_min_size(
            origin + vec2(node.x, node.y) * zoom,
            vec2(node.width, node.height) * zoom,
        );
        if !rect.intersects(content.clip_rect()) {
            continue;
        }
        match node.kind {
            NodeKind::Rectangle => {
                content.rect_filled(
                    rect,
                    (node.radius.min(node.width.min(node.height) / 2.0) * zoom).min(255.0) as u8,
                    color(document.node_fill(node)),
                );
            }
            NodeKind::Text if editing == Some(node.id) => {}
            NodeKind::Text => {
                let mut size = node.font_size * zoom;
                if preview {
                    if size < 2.0 {
                        continue;
                    }
                    size = (size * 2.0).round() / 2.0;
                }
                let fill = color(document.node_fill(node));
                let galley = content.layout(
                    node.text.clone(),
                    FontId::proportional(size),
                    fill,
                    rect.width(),
                );
                content
                    .with_clip_rect(rect.intersect(artboard).intersect(clip))
                    .galley(rect.min, galley, fill);
            }
        }
    }
}

/// Straight-alpha RGBA pixels, row-major from the top left.
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Pixel dimensions of the artboard at `scale`, or an error when the scale or the
/// resulting image is outside the PNG export limits.
pub fn png_size(document: &Document, scale: f32) -> Result<(u32, u32), String> {
    if !scale.is_finite() || !PNG_SCALE.contains(&scale) {
        return Err(format!(
            "PNG scale must be between {} and {}.",
            PNG_SCALE.start(),
            PNG_SCALE.end()
        ));
    }
    let side = |n: f32| (n * scale).round().max(1.0);
    let (width, height) = (side(document.width), side(document.height));
    if width > MAX_PNG_SIDE as f32 || height > MAX_PNG_SIDE as f32 {
        return Err(format!(
            "PNG export is limited to {MAX_PNG_SIDE} pixels per side; this artboard is {width}×{height} at {scale}×."
        ));
    }
    let (width, height) = (width as u32, height as u32);
    if u64::from(width) * u64::from(height) > MAX_PNG_PIXELS {
        return Err(format!(
            "PNG export is limited to {MAX_PNG_PIXELS} pixels; this artboard is {width}×{height} at {scale}×."
        ));
    }
    Ok((width, height))
}

/// Renders the artboard at `scale` pixels per document unit, independent of any
/// window, zoom, pan or display density.
pub fn rasterize(document: &Document, scale: f32) -> Result<Raster, String> {
    document.validate()?;
    let (width, height) = png_size(document, scale)?;
    let ctx = egui::Context::default();
    ctx.set_fonts(font_definitions());
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32));
    let mut textures: HashMap<TextureId, egui::ColorImage> = HashMap::new();
    let mut shapes = Vec::new();
    // Fonts load during the first pass; paint on the second so galleys use them.
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let painter = ui.ctx().layer_painter(LayerId::background());
            paint_document(&painter, document, Pos2::ZERO, scale, screen, false, None);
        });
        for (id, deltas) in std::mem::take(&mut output.textures_delta.set) {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                match delta.pos {
                    None => {
                        textures.insert(id, (**image).clone());
                    }
                    Some([x, y]) => {
                        if let Some(target) = textures.get_mut(&id) {
                            for row in 0..image.size[1] {
                                for col in 0..image.size[0] {
                                    let (tx, ty) = (x + col, y + row);
                                    if tx < target.size[0] && ty < target.size[1] {
                                        target.pixels[ty * target.size[0] + tx] =
                                            image.pixels[row * image.size[0] + col];
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        shapes = output.shapes;
    }
    // The root Ui paints nothing of its own; keep only the artboard layer.
    shapes.retain(|s| s.clip_rect.is_positive());
    let primitives = ctx.tessellate(shapes, 1.0);
    let mut buffer = vec![[0.0f32; 4]; width as usize * height as usize];
    for primitive in primitives {
        let Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let texture = textures.get(&mesh.texture_id);
        let clip = primitive.clip_rect.intersect(screen);
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v = tri.map(|i| &mesh.vertices[i as usize]);
            fill_triangle(&mut buffer, width, height, clip, v, texture);
        }
    }
    let mut pixels = Vec::with_capacity(buffer.len() * 4);
    for [r, g, b, a] in buffer {
        let unpremultiply = |c: f32| {
            if a > 0.0 {
                (c / a * 255.0).round().clamp(0.0, 255.0) as u8
            } else {
                0
            }
        };
        pixels.extend([
            unpremultiply(r),
            unpremultiply(g),
            unpremultiply(b),
            (a * 255.0).round().clamp(0.0, 255.0) as u8,
        ]);
    }
    Ok(Raster {
        width,
        height,
        pixels,
    })
}

/// Encodes the artboard as PNG bytes.
pub fn export_png(document: &Document, scale: f32) -> Result<Vec<u8>, String> {
    let raster = rasterize(document, scale)?;
    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, raster.width, raster.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        writer
            .write_image_data(&raster.pixels)
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
    }
    Ok(data)
}

/// Premultiplied components of a sRGB color, as egui paints them.
fn premultiplied(c: Color32) -> [f32; 4] {
    let [r, g, b, a] = c.to_array();
    [r, g, b, a].map(|n| f32::from(n) / 255.0)
}

/// Bilinearly samples a premultiplied texture at normalized `uv`.
fn sample(texture: &egui::ColorImage, uv: Pos2) -> [f32; 4] {
    let [w, h] = texture.size;
    let x = (uv.x * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let y = (uv.y * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: usize, y: usize| premultiplied(texture.pixels[y * w + x]);
    let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    std::array::from_fn(|i| {
        let top = a[i] + (b[i] - a[i]) * fx;
        let bottom = c[i] + (d[i] - c[i]) * fx;
        top + (bottom - top) * fy
    })
}

fn fill_triangle(
    buffer: &mut [[f32; 4]],
    width: u32,
    height: u32,
    clip: Rect,
    v: [&egui::epaint::Vertex; 3],
    texture: Option<&egui::ColorImage>,
) {
    let [a, b, c] = v.map(|v| v.pos);
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < f32::EPSILON {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).max(clip.min.x).max(0.0).floor() as i64;
    let max_x =
        a.x.max(b.x)
            .max(c.x)
            .min(clip.max.x)
            .min(width as f32)
            .ceil() as i64;
    let min_y = a.y.min(b.y).min(c.y).max(clip.min.y).max(0.0).floor() as i64;
    let max_y =
        a.y.max(b.y)
            .max(c.y)
            .min(clip.max.y)
            .min(height as f32)
            .ceil() as i64;
    let colors = v.map(|v| premultiplied(v.color));
    let edge = |p: Pos2, q: Pos2, r: Pos2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    // Include only top/left edges so shared edges are shaded once, even with alpha.
    let owns_edge = |mut p: Pos2, mut q: Pos2| {
        if area < 0.0 {
            std::mem::swap(&mut p, &mut q);
        }
        q.y < p.y || (q.y == p.y && q.x > p.x)
    };
    let inclusive = [owns_edge(b, c), owns_edge(c, a), owns_edge(a, b)];
    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
            if !clip.contains(p) {
                continue;
            }
            let (w0, w1, w2) = (
                edge(b, c, p) / area,
                edge(c, a, p) / area,
                edge(a, b, p) / area,
            );
            if [w0, w1, w2]
                .into_iter()
                .zip(inclusive)
                .any(|(weight, inclusive)| weight < 0.0 || (weight == 0.0 && !inclusive))
            {
                continue;
            }
            let mut src: [f32; 4] =
                std::array::from_fn(|i| colors[0][i] * w0 + colors[1][i] * w1 + colors[2][i] * w2);
            if let Some(texture) = texture {
                let uv = pos2(
                    v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2,
                    v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2,
                );
                let texel = sample(texture, uv);
                src = std::array::from_fn(|i| src[i] * texel[i]);
            }
            let dst = &mut buffer[y as usize * width as usize + x as usize];
            let keep = 1.0 - src[3];
            for i in 0..4 {
                dst[i] = src[i] + dst[i] * keep;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_edges_are_owned_once_for_both_windings_and_diagonals() {
        let vertices =
            [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]].map(|[x, y]| egui::epaint::Vertex {
                pos: pos2(x, y),
                uv: Pos2::ZERO,
                color: color([80, 120, 160, 128]),
            });
        for triangles in [[[0, 1, 2], [0, 2, 3]], [[0, 1, 3], [1, 2, 3]]] {
            for reversed in [false, true] {
                let mut buffer = vec![[0.0; 4]; 16];
                for mut triangle in triangles {
                    if reversed {
                        triangle.swap(0, 2);
                    }
                    fill_triangle(
                        &mut buffer,
                        4,
                        4,
                        Rect::from_min_max(Pos2::ZERO, pos2(4.0, 4.0)),
                        triangle.map(|i| &vertices[i]),
                        None,
                    );
                }
                for pixel in buffer {
                    assert!((pixel[3] - 128.0 / 255.0).abs() < 0.00001, "{pixel:?}");
                }
            }
        }
    }
}
