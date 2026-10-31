//! Bounded, local Figma import. The source is never a save destination.
#[allow(dead_code)]
mod kiwi;

use crate::document::{Document, MAX_DOCUMENT_BYTES, Node, NodeKind};
use kiwi::{Kv, Schema};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

const MAX_INFLATED: u64 = 64 * 1024 * 1024;

pub struct Import {
    pub document: Document,
    pub warnings: Vec<String>,
}

pub fn is_fig(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fig"))
}

fn capped(reader: impl Read, limit: u64) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("Figma read failed: {e}"))?;
    if out.len() as u64 > limit {
        return Err("Figma input exceeds the import size limit.".into());
    }
    Ok(out)
}

pub fn load(path: &Path) -> Result<Import, String> {
    let bytes = capped(
        File::open(path).map_err(|e| format!("Open failed: {e}"))?,
        MAX_DOCUMENT_BYTES,
    )?;
    import(
        &bytes,
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Figma"),
    )
}

/// Accept a ZIP containing canvas.fig, or a bare fig-kiwi canvas.
pub fn import(bytes: &[u8], name: &str) -> Result<Import, String> {
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("Figma input exceeds 16 MiB.".into());
    }
    let canvas = if bytes.starts_with(b"PK") {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| format!("Invalid Figma archive: {e}"))?;
        let member = archive
            .by_name("canvas.fig")
            .map_err(|e| format!("Missing canvas.fig: {e}"))?;
        capped(member, MAX_INFLATED)?
    } else {
        bytes.to_vec()
    };
    if !canvas.starts_with(b"fig-kiwi") || canvas.len() < 12 {
        return Err("Expected a Figma fig-kiwi canvas.".into());
    }
    let mut pos = 12usize;
    let mut chunks = Vec::new();
    while pos < canvas.len() {
        if chunks.len() == 16 {
            return Err("Too many Figma chunks.".into());
        }
        let length = canvas
            .get(pos..pos + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or("Truncated Figma chunk length.")? as usize;
        pos += 4;
        let end = pos
            .checked_add(length)
            .filter(|e| *e <= canvas.len())
            .ok_or("Truncated Figma chunk.")?;
        chunks.push(&canvas[pos..end]);
        pos = end;
    }
    if chunks.len() < 2 {
        return Err("Figma canvas needs schema and message chunks.".into());
    }
    let inflate = |data: &[u8]| -> Result<Vec<u8>, String> {
        if data.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            let decoder = ruzstd::decoding::StreamingDecoder::new(data)
                .map_err(|e| format!("Invalid Figma zstd: {e}"))?;
            capped(decoder, MAX_INFLATED)
        } else {
            capped(flate2::read::DeflateDecoder::new(data), MAX_INFLATED)
                .or_else(|_| capped(flate2::read::ZlibDecoder::new(data), MAX_INFLATED))
        }
    };
    let schema = Schema::decode(&inflate(chunks[0])?).map_err(|e| e.to_string())?;
    let root = schema
        .find("Message")
        .ok_or("Figma schema has no Message.")?;
    let data = inflate(chunks[1])?;
    let message = schema
        .decode_message_within(root, &data, 1_000_000)
        .map_err(|e| format!("Invalid Figma message: {e}"))?;
    map(&message, name)
}

fn number(value: &Kv, key: &str, default: f32) -> f32 {
    value
        .get(key)
        .and_then(Kv::f64)
        .map_or(default, |n| n as f32)
}
fn text<'a>(value: &'a Kv, key: &str) -> &'a str {
    value.get(key).and_then(Kv::str).unwrap_or("")
}
fn guid(value: &Kv) -> Option<(u64, u64)> {
    match (value.get("sessionID")?, value.get("localID")?) {
        (Kv::Uint(a), Kv::Uint(b)) => Some((*a, *b)),
        _ => None,
    }
}
fn color(row: &Kv) -> [u8; 4] {
    let paints = row.get("fillPaints").map(Kv::list).unwrap_or_default();
    let Some(paint) = paints
        .iter()
        .find(|p| text(p, "type") == "SOLID" && p.get("visible").and_then(Kv::bool) != Some(false))
    else {
        return [0; 4];
    };
    let Some(c) = paint.get("color") else {
        return [0; 4];
    };
    let channel = |k| (number(c, k, 0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        channel("r"),
        channel("g"),
        channel("b"),
        (number(c, "a", 1.0) * number(paint, "opacity", 1.0) * number(row, "opacity", 1.0))
            .clamp(0.0, 1.0)
            .mul_add(255.0, 0.0)
            .round() as u8,
    ]
}

fn map(message: &Kv, name: &str) -> Result<Import, String> {
    let rows = message
        .get("nodeChanges")
        .ok_or("Figma message has no nodeChanges.")?
        .list();
    if rows.len() > 10_000 {
        return Err("Figma import exceeds 10,000 source nodes.".into());
    }
    let mut ids = HashSet::new();
    let mut children = HashMap::new();
    for row in rows {
        let id = row
            .get("guid")
            .and_then(guid)
            .ok_or("Figma node has no valid ID.")?;
        if !ids.insert(id) {
            return Err("Duplicate Figma node ID.".into());
        }
        if let Some(parent) = row
            .get("parentIndex")
            .and_then(|p| p.get("guid"))
            .and_then(guid)
        {
            children.entry(parent).or_insert_with(Vec::new).push(row);
        }
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|row| {
            row.get("parentIndex")
                .map(|p| text(p, "position"))
                .unwrap_or("")
        });
    }
    let page = rows
        .iter()
        .filter(|r| text(r, "type") == "CANVAS")
        .min_by_key(|r| {
            r.get("parentIndex")
                .map(|p| text(p, "position"))
                .unwrap_or("")
        })
        .ok_or("Figma file contains no page.")?;
    let mut document = Document {
        name: name.to_owned(),
        background: [255; 4],
        ..Document::default()
    };
    let mut skipped = 0;
    // shortcut: first page and static primitives only; expand after adding richer document types.
    fn visit(
        row: &Kv,
        parent: Option<u64>,
        offset: [f32; 2],
        depth: usize,
        children: &HashMap<(u64, u64), Vec<&Kv>>,
        doc: &mut Document,
        skipped: &mut usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("Figma hierarchy is cyclic or exceeds 64 levels.".into());
        }
        let kind = match text(row, "type") {
            "FRAME" | "GROUP" | "SYMBOL" => NodeKind::Frame,
            "RECTANGLE" | "ROUNDED_RECTANGLE" => NodeKind::Rectangle,
            "ELLIPSE" => NodeKind::Ellipse,
            "TEXT" => NodeKind::Text,
            _ => {
                *skipped += 1;
                return Ok(());
            }
        };
        let mut xy = offset;
        if let Some(t) = row.get("transform") {
            if number(t, "m00", 1.0) != 1.0
                || number(t, "m11", 1.0) != 1.0
                || number(t, "m01", 0.0) != 0.0
                || number(t, "m10", 0.0) != 0.0
            {
                *skipped += 1;
                return Ok(());
            }
            xy[0] += number(t, "m02", 0.0);
            xy[1] += number(t, "m12", 0.0);
        }
        let size = row.get("size").ok_or("Figma layer has no size.")?;
        let id = doc.next_id;
        doc.next_id += 1;
        let node = Node {
            id,
            kind,
            name: text(row, "name").to_owned(),
            parent,
            x: xy[0],
            y: xy[1],
            width: number(size, "x", 0.0),
            height: number(size, "y", 0.0),
            fill: color(row),
            radius: number(row, "cornerRadius", 0.0),
            font_size: number(row, "fontSize", 28.0),
            text: if kind == NodeKind::Text {
                row.get("textData")
                    .map(|t| text(t, "characters").to_owned())
                    .unwrap_or_default()
            } else {
                String::new()
            },
            hidden: row.get("visible").and_then(Kv::bool) == Some(false),
            locked: row.get("locked").and_then(Kv::bool) == Some(true),
            clip: kind == NodeKind::Frame
                && text(row, "type") != "GROUP"
                && !row
                    .get("frameMaskDisabled")
                    .and_then(Kv::bool)
                    .unwrap_or(false),
            ..Node::default()
        };
        doc.nodes.push(node);
        if kind == NodeKind::Frame {
            let key = row.get("guid").and_then(guid).ok_or("Missing Figma ID.")?;
            for child in children.get(&key).into_iter().flatten() {
                visit(child, Some(id), xy, depth + 1, children, doc, skipped)?;
            }
        }
        Ok(())
    }
    let page_id = page
        .get("guid")
        .and_then(guid)
        .ok_or("Missing Figma page ID.")?;
    for row in children.get(&page_id).into_iter().flatten() {
        visit(
            row,
            None,
            [0.0; 2],
            0,
            &children,
            &mut document,
            &mut skipped,
        )?;
    }
    if document.nodes.is_empty() {
        return Err("No supported layers on the first Figma page.".into());
    }
    document.width = document
        .nodes
        .iter()
        .map(|n| n.x + n.width)
        .fold(1.0, f32::max);
    document.height = document
        .nodes
        .iter()
        .map(|n| n.y + n.height)
        .fold(1.0, f32::max);
    document.version = crate::document::CURRENT_VERSION;
    document.normalize();
    document
        .validate()
        .map_err(|e| format!("Converted Figma document is outside PaperCraft limits: {e}"))?;
    Ok(Import {
        document,
        warnings: vec![format!(
            "Best-effort Figma import: first page only; {skipped} unsupported layer/subtree(s) skipped. Only static geometry, first solid fill and plain text are retained. Images, vectors, instances, gradients, strokes, effects, layout rules, typography, variables and prototypes are not converted; group opacity is not composited."
        )],
    })
}

#[cfg(test)]
pub(crate) mod tests;
