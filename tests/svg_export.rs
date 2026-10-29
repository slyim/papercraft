use papercraft::document::{Document, atomic_write};

fn fixture() -> Document {
    serde_json::from_str(include_str!("fixtures/svg-exchange.papercraft")).unwrap()
}

#[test]
fn svg_exchange_preserves_visible_document_content_and_leaves_the_source_editable() {
    let document = fixture();
    let svg = document.export_svg().unwrap();
    assert!(svg.contains("viewBox=\"0 0 640 400\""));
    assert!(svg.contains("SVG exchange &lt;&amp;&gt; &quot;İstanbul&quot;"));
    assert!(svg.contains("İstanbul · café &lt;&amp;&gt; &quot;quotes&quot;"));
    assert!(svg.contains("第二行 · Ελληνικά"));
    assert!(svg.contains("fill-opacity=\"0.5020\""));
    assert!(svg.contains("rx=\"70\""), "Radius fits the rectangle");
    assert!(svg.contains("clip-path=\"url(#clip-5)\""));
    assert!(!svg.contains("Hidden sentinel"));
    assert!(!svg.contains("layer-4"));
    let positions: Vec<_> = ["layer-1", "layer-2", "layer-3", "layer-5"]
        .map(|id| svg.find(id).unwrap())
        .into();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!svg.contains("<script"));
    assert!(!svg.contains("foreignObject"));

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("exchange.papercraft");
    document.save(&path).unwrap();
    let reopened = Document::load(&path).unwrap();
    assert_eq!(reopened, document);
    assert_eq!(reopened.export_svg().unwrap(), svg);
    atomic_write(&directory.path().join("exchange.svg"), svg.as_bytes()).unwrap();
    assert_eq!(
        document.nodes.len(),
        5,
        "Export does not flatten the source"
    );
}

#[test]
fn svg_rejects_invalid_xml_and_geometry_before_replacing_an_existing_export() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("keep.svg");
    atomic_write(&path, b"existing export").unwrap();
    let mut document = fixture();
    document.nodes[2].text.push('\u{1}');
    document.validate().unwrap(); // Version 1 JSON permits this; XML does not.
    assert!(
        document
            .export_svg()
            .and_then(|svg| atomic_write(&path, svg.as_bytes()))
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"existing export");
    document = fixture();
    document.nodes[0].x = f32::NAN;
    assert!(document.export_svg().is_err());
}

#[test]
fn svg_and_html_export_ellipse_and_triangle_shapes() {
    use papercraft::document::NodeKind;
    let mut document = fixture();
    document.nodes[0].kind = NodeKind::Ellipse;
    document.nodes[0].radius = 0.0;
    document.nodes[1].kind = NodeKind::Triangle;
    document.nodes[1].radius = 0.0;
    document.normalize();
    assert_eq!(document.version, 3);
    let svg = document.export_svg().unwrap();
    assert!(svg.contains("<ellipse id=\"layer-1\""));
    assert!(svg.contains("<polygon id=\"layer-2\""));
    let html = document.export_html().unwrap();
    assert!(html.contains("class=\"layer ellipse\""));
    assert!(html.contains("border-radius:50%"));
    assert!(html.contains("class=\"layer triangle\""));
    assert!(html.contains("clip-path:polygon(50% 0%,100% 100%,0% 100%)"));
}
