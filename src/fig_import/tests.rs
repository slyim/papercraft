use super::*;
use kiwi::encode;
use std::io::Write;

const SCHEMA: &str = "
enum NodeType CANVAS FRAME RECTANGLE ELLIPSE TEXT VECTOR
struct GUID uint sessionID, uint localID
struct Parent GUID guid, string position
struct Vector float x, float y
struct Matrix float m00, float m01, float m02, float m10, float m11, float m12
struct Color float r, float g, float b, float a
enum PaintType SOLID IMAGE
message Paint PaintType type, Color color, float opacity
message TextData string characters
message Node GUID guid, Parent parentIndex, NodeType type, string name, Vector size, Matrix transform, Paint[] fillPaints, TextData textData, float fontSize, bool frameMaskDisabled
message Message Node[] nodeChanges
";
fn obj(fields: Vec<(&'static str, Kv<'static>)>) -> Kv<'static> {
    Kv::Object(fields)
}
fn id(n: u64) -> Kv<'static> {
    obj(vec![("sessionID", Kv::Uint(1)), ("localID", Kv::Uint(n))])
}
fn row(n: u64, parent: u64, position: &str, kind: &'static str, x: f32) -> Kv<'static> {
    obj(vec![
        ("guid", id(n)),
        (
            "parentIndex",
            obj(vec![
                ("guid", id(parent)),
                ("position", Kv::Str(position.into())),
            ]),
        ),
        ("type", Kv::Enum(kind)),
        ("name", Kv::Str(format!("Layer {n}"))),
        (
            "size",
            obj(vec![("x", Kv::Float(100.0)), ("y", Kv::Float(60.0))]),
        ),
        (
            "transform",
            obj(vec![
                ("m00", Kv::Float(1.0)),
                ("m01", Kv::Float(0.0)),
                ("m02", Kv::Float(x)),
                ("m10", Kv::Float(0.0)),
                ("m11", Kv::Float(1.0)),
                ("m12", Kv::Float(20.0)),
            ]),
        ),
        (
            "fillPaints",
            Kv::Array(vec![obj(vec![
                ("type", Kv::Enum("SOLID")),
                (
                    "color",
                    obj(vec![
                        ("r", Kv::Float(1.0)),
                        ("g", Kv::Float(0.0)),
                        ("b", Kv::Float(0.0)),
                        ("a", Kv::Float(1.0)),
                    ]),
                ),
                ("opacity", Kv::Float(0.5)),
            ])]),
        ),
        (
            "textData",
            obj(vec![("characters", Kv::Str("Merhaba 世界".into()))]),
        ),
        ("fontSize", Kv::Float(24.0)),
    ])
}
fn message() -> Kv<'static> {
    obj(vec![(
        "nodeChanges",
        Kv::Array(vec![
            row(1, 0, "a", "CANVAS", 0.0),
            row(2, 1, "a", "FRAME", 30.0),
            row(4, 2, "b", "TEXT", 10.0),
            row(3, 2, "a", "RECTANGLE", 5.0),
            row(5, 1, "b", "VECTOR", 0.0),
        ]),
    )])
}
fn canvas(message: &Kv, zlib: bool) -> Vec<u8> {
    let schema = Schema::parse(SCHEMA).unwrap();
    let mut out = b"fig-kiwi".to_vec();
    out.extend(15u32.to_le_bytes());
    for chunk in [
        encode::schema(&schema),
        encode::message(&schema, schema.find("Message").unwrap(), message).unwrap(),
    ] {
        let data = if zlib {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&chunk).unwrap();
            encoder.finish().unwrap()
        } else {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&chunk).unwrap();
            encoder.finish().unwrap()
        };
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
    }
    out
}

#[test]
fn imports_binary_and_zip_then_saves_native_document() {
    for zlib in [false, true] {
        let bytes = canvas(&message(), zlib);
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            "canvas.fig",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
        let archive = zip.finish().unwrap().into_inner();
        for source in [&bytes, &archive] {
            let imported = import(source, "Design").unwrap();
            let doc = imported.document;
            assert_eq!(doc.nodes.len(), 3);
            assert_eq!(
                doc.nodes
                    .iter()
                    .map(|n| n.name.as_str())
                    .collect::<Vec<_>>(),
                ["Layer 2", "Layer 3", "Layer 4"]
            );
            assert_eq!([doc.nodes[1].x, doc.nodes[1].y], [35.0, 40.0]);
            assert_eq!(doc.nodes[1].parent, Some(doc.nodes[0].id));
            assert_eq!(doc.nodes[1].fill, [255, 0, 0, 128]);
            assert_eq!(doc.nodes[2].text, "Merhaba 世界");
            assert!(imported.warnings[0].contains("1 unsupported"));
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("out.papercraft");
            doc.save(&path).unwrap();
            let mut expected = doc;
            expected.normalize();
            assert_eq!(Document::load(&path).unwrap(), expected);
        }
    }
}

#[test]
fn rejects_truncation_duplicates_and_oversized_expansion() {
    let bytes = canvas(&message(), false);
    for n in 0..bytes.len() {
        assert!(import(&bytes[..n], "broken").is_err(), "prefix {n}");
    }
    let mut bad = message();
    if let Kv::Object(fields) = &mut bad
        && let Kv::Array(rows) = &mut fields[0].1
    {
        rows.push(rows[0].clone());
    }
    assert!(map(&bad, "duplicate").err().unwrap().contains("Duplicate"));
    assert!(capped(Cursor::new(vec![0; 101]), 100).is_err());
}

#[test]
fn conversion_preserves_source_and_opens_native_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("design.fig");
    let output = dir.path().join("design.papercraft");
    let bytes = canvas(&message(), false);
    std::fs::write(&source, &bytes).unwrap();
    let args = [
        "convert".into(),
        source.as_os_str().to_owned(),
        "--output".into(),
        output.as_os_str().to_owned(),
    ];
    assert_eq!(crate::cli::run(&args), Some(0));
    assert_eq!(std::fs::read(source).unwrap(), bytes);
    assert_eq!(Document::load(&output).unwrap().nodes.len(), 3);
}

pub(crate) fn fixture() -> Vec<u8> {
    canvas(&message(), false)
}

#[test]
fn imports_zstd_raw_blocks() {
    let schema = Schema::parse(SCHEMA).unwrap();
    let mut canvas = b"fig-kiwi".to_vec();
    canvas.extend(15u32.to_le_bytes());
    for chunk in [
        encode::schema(&schema),
        encode::message(&schema, schema.find("Message").unwrap(), &message()).unwrap(),
    ] {
        // A single-segment zstd frame with one final uncompressed block.
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0xa0];
        frame.extend((chunk.len() as u32).to_le_bytes());
        let header = ((chunk.len() as u32) << 3) | 1;
        frame.extend(&header.to_le_bytes()[..3]);
        frame.extend(chunk);
        canvas.extend((frame.len() as u32).to_le_bytes());
        canvas.extend(frame);
    }
    assert_eq!(import(&canvas, "zstd").unwrap().document.nodes.len(), 3);
}

#[test]
fn first_page_follows_sibling_order_and_rotated_subtrees_are_skipped() {
    let mut input = message();
    if let Kv::Object(fields) = &mut input
        && let Kv::Array(rows) = &mut fields[0].1
    {
        rows.insert(0, row(10, 0, "z", "CANVAS", 0.0));
        rows.push(row(11, 10, "a", "ELLIPSE", 0.0));
    }
    assert_eq!(map(&input, "pages").unwrap().document.nodes.len(), 3);
    if let Kv::Object(fields) = &mut input
        && let Kv::Array(rows) = &mut fields[0].1
        && let Kv::Object(fields) = &mut rows[2]
        && let Some((_, Kv::Object(matrix))) = fields.iter_mut().find(|(k, _)| *k == "transform")
    {
        matrix[0].1 = Kv::Float(0.0);
    }
    assert!(
        map(&input, "rotated")
            .err()
            .unwrap()
            .contains("No supported layers")
    );
}
