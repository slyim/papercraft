use papercraft::document::{Document, Node, NodeKind};
use papercraft::render::{MAX_PNG_SIDE, export_png, png_size, rasterize};

fn rect(id: u64, x: f32, y: f32, w: f32, h: f32, fill: [u8; 4]) -> Node {
    Node {
        id,
        name: format!("R{id}"),
        kind: NodeKind::Rectangle,
        x,
        y,
        width: w,
        height: h,
        fill,
        radius: 0.0,
        text: String::new(),
        font_size: 12.0,
        hidden: false,
        locked: false,
        fill_token: None,
        line_height: None,
        text_align: papercraft::document::TextAlign::Left,
    }
}

fn document(nodes: Vec<Node>) -> Document {
    let next_id = nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
    Document {
        width: 40.0,
        height: 20.0,
        background: [255, 255, 255, 255],
        nodes,
        next_id,
        ..Document::default()
    }
}

fn pixel(raster: &papercraft::render::Raster, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * raster.width + x) * 4) as usize;
    raster.pixels[i..i + 4].try_into().unwrap()
}

#[test]
fn png_size_follows_scale_not_any_viewport() {
    let doc = document(Vec::new());
    assert_eq!(png_size(&doc, 1.0).unwrap(), (40, 20));
    assert_eq!(png_size(&doc, 2.0).unwrap(), (80, 40));
    assert_eq!(png_size(&doc, 0.5).unwrap(), (20, 10));
    let raster = rasterize(&doc, 3.0).unwrap();
    assert_eq!((raster.width, raster.height), (120, 60));
    assert_eq!(raster.pixels.len(), 120 * 60 * 4);
}

#[test]
fn paint_order_alpha_and_hidden_layers_follow_the_document() {
    let mut hidden = rect(3, 0.0, 0.0, 40.0, 20.0, [0, 0, 0, 255]);
    hidden.hidden = true;
    let doc = document(vec![
        rect(1, 0.0, 0.0, 20.0, 20.0, [255, 0, 0, 255]),
        // Half-transparent blue over the right half of the red square and the background.
        rect(2, 10.0, 0.0, 20.0, 20.0, [0, 0, 255, 128]),
        hidden,
    ]);
    let raster = rasterize(&doc, 1.0).unwrap();
    assert_eq!(pixel(&raster, 4, 10), [255, 0, 0, 255], "red only");
    let [r, g, b, a] = pixel(&raster, 15, 10);
    assert!(
        r.abs_diff(127) <= 2 && g == 0 && b.abs_diff(128) <= 2 && a == 255,
        "{r} {g} {b} {a}"
    );
    let [r, g, b, a] = pixel(&raster, 25, 10);
    assert!(r.abs_diff(127) <= 2 && g.abs_diff(127) <= 2 && b == 255 && a == 255);
    assert_eq!(
        pixel(&raster, 35, 10),
        [255, 255, 255, 255],
        "hidden layer is omitted"
    );
}

#[test]
fn transparent_artboard_keeps_alpha_and_layers_clip_to_it() {
    let mut doc = document(vec![rect(1, 30.0, 0.0, 30.0, 20.0, [0, 128, 0, 255])]);
    doc.background = [0, 0, 0, 0];
    let raster = rasterize(&doc, 1.0).unwrap();
    assert_eq!(
        pixel(&raster, 5, 5)[3],
        0,
        "transparent background stays transparent"
    );
    assert_eq!(pixel(&raster, 39, 5), [0, 128, 0, 255]);
    assert_eq!(
        (raster.width, raster.height),
        (40, 20),
        "overflow is clipped"
    );
}

#[test]
fn translucent_surfaces_have_no_triangle_seams() {
    for background in [true, false] {
        let mut doc = document(Vec::new());
        doc.height = doc.width;
        doc.background = [0, 0, 0, 0];
        if background {
            doc.background = [80, 120, 160, 128];
        } else {
            doc.nodes
                .push(rect(1, 0.0, 0.0, 40.0, 40.0, [80, 120, 160, 128]));
            doc.next_id = 2;
        }
        let raster = rasterize(&doc, 1.0).unwrap();
        for y in 2..38 {
            for x in 2..38 {
                assert_eq!(
                    pixel(&raster, x, y)[3],
                    128,
                    "background={background}, ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn bound_token_colors_are_resolved() {
    let tokens: Document =
        serde_json::from_str(include_str!("fixtures/tokens-v2.papercraft")).unwrap();
    let raster = rasterize(&tokens, 1.0).unwrap();
    let background = tokens.background_color();
    assert_eq!(pixel(&raster, 0, 0), background);
}

#[test]
fn text_renders_glyph_pixels_inside_its_layer() {
    let mut text = rect(1, 2.0, 2.0, 36.0, 16.0, [0, 0, 0, 255]);
    text.kind = NodeKind::Text;
    text.text = "Ag".into();
    text.font_size = 14.0;
    let raster = rasterize(&document(vec![text]), 2.0).unwrap();
    let dark = (0..raster.height)
        .flat_map(|y| (0..raster.width).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&raster, x, y)[0] < 128)
        .collect::<Vec<_>>();
    assert!(dark.len() > 20, "glyphs are painted");
    assert!(
        dark.iter().all(|&(x, y)| x >= 4 && y >= 4),
        "text stays inside its layer"
    );
}

#[test]
fn oversized_and_invalid_scales_are_rejected_before_rendering() {
    let mut doc = document(Vec::new());
    for scale in [0.0, -1.0, 0.05, 8.5, f32::NAN, f32::INFINITY] {
        assert!(png_size(&doc, scale).is_err(), "{scale}");
    }
    doc.width = (MAX_PNG_SIDE + 1) as f32;
    assert!(png_size(&doc, 1.0).unwrap_err().contains("per side"));
    doc.width = 10_000.0;
    doc.height = 10_000.0;
    assert!(png_size(&doc, 1.0).unwrap_err().contains("pixels"));
    assert!(export_png(&doc, 1.0).is_err());
}

#[test]
fn png_bytes_are_a_valid_rgba_image() {
    let data = export_png(&document(Vec::new()), 1.0).unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let reader = decoder.read_info().unwrap();
    let info = reader.info();
    assert_eq!((info.width, info.height), (40, 20));
    assert_eq!(info.color_type, png::ColorType::Rgba);
}
