use eframe::egui::{pos2, vec2};
use papercraft::{
    document::{Document, Editor, Node, NodeKind},
    mcp::Server,
    vector::{self, Anchor, PointMode},
};
use serde_json::{Value, json};
fn curve() -> Node {
    let mut n = Node {
        kind: NodeKind::Path,
        fill: [0; 4],
        stroke: Some([255, 80, 30, 255]),
        stroke_width: 3.0,
        ..Node::default()
    };
    let mut a = Anchor::corner(pos2(-80.0, 40.0));
    a.outgoing = Some([20.0, -100.0]);
    let mut b = Anchor::corner(pos2(300.0, 170.0));
    b.incoming = Some([180.0, 310.0]);
    vector::set_world_anchors(&mut n, &[a, b]).unwrap();
    n
}
#[test]
fn cubic_bounds_flattening_split_and_zero_span_rebasing_preserve_geometry() {
    let n = curve();
    let a = vector::world_anchors(&n);
    let b = vector::world_bounds(&a, false);
    for i in 0..=1000 {
        let p = vector::cubic(
            [
                a[0].pos(),
                pos2(20.0, -100.0),
                pos2(180.0, 310.0),
                a[1].pos(),
            ],
            i as f32 / 1000.0,
        );
        assert!(
            p.x >= b[0] - 0.01 && p.x <= b[2] + 0.01 && p.y >= b[1] - 0.01 && p.y <= b[3] + 0.01
        );
    }
    let mut split = a.clone();
    vector::split_segment(&mut split, 0, 0.37, false).unwrap();
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let before = vector::cubic(
            [
                a[0].pos(),
                pos2(20.0, -100.0),
                pos2(180.0, 310.0),
                a[1].pos(),
            ],
            t,
        );
        let (index, t) = if t <= 0.37 {
            (0, t / 0.37)
        } else {
            (1, (t - 0.37) / 0.63)
        };
        let x = &split[index];
        let y = &split[index + 1];
        let p = |p: [f32; 2]| pos2(p[0], p[1]);
        let after = vector::cubic(
            [
                x.pos(),
                p(x.outgoing.unwrap()),
                p(y.incoming.unwrap()),
                y.pos(),
            ],
            t,
        );
        assert!(before.distance(after) < 0.001);
    }
    assert!(vector::flatten(&n).unwrap().len() > 10);
    let mut vertical = Node {
        kind: NodeKind::Path,
        ..Node::default()
    };
    let a = vec![
        Anchor::corner(pos2(-20.0, 3.0)),
        Anchor::corner(pos2(-20.0, 90.0)),
    ];
    vector::set_world_anchors(&mut vertical, &a).unwrap();
    assert_eq!(vertical.width, 1.0);
    assert_eq!(vector::world_anchors(&vertical), a);
}
#[test]
fn mirrored_smooth_and_independent_handles_have_distinct_behavior() {
    let mut a = Anchor::corner(pos2(50.0, 50.0));
    a.mode = PointMode::Symmetric;
    a.set_handle(false, pos2(70.0, 40.0), false);
    assert_eq!(a.incoming, Some([30.0, 60.0]));
    a.mode = PointMode::Smooth;
    let old = pos2(30.0, 60.0).distance(a.pos());
    a.set_handle(false, pos2(90.0, 50.0), false);
    assert!(
        (pos2(a.incoming.unwrap()[0], a.incoming.unwrap()[1]).distance(a.pos()) - old).abs()
            < 0.001
    );
    let incoming = a.incoming;
    a.set_handle(false, pos2(70.0, 80.0), true);
    assert_eq!(a.incoming, incoming);
    assert_eq!(a.mode, PointMode::Corner);
    a.translate(vec2(-20.0, 10.0));
    assert_eq!(a.position, [30.0, 60.0]);
}
#[test]
fn v4_roundtrips_and_frozen_schemas_reject_even_default_anchor_keys() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("curves.papercraft");
    let mut d = Document {
        nodes: vec![curve()],
        next_id: 2,
        ..Document::default()
    };
    d.normalize();
    assert_eq!(d.version, 4);
    d.save(&p).unwrap();
    assert_eq!(Document::load(&p).unwrap(), d);
    assert!(d.export_svg().unwrap().contains(" C "));
    assert!(d.export_html().unwrap().contains("fill-rule=\"evenodd\""));
    for v in 1..=3 {
        let mut value = serde_json::to_value(Document::default()).unwrap();
        value["version"] = json!(v);
        let mut n = serde_json::to_value(Node::default()).unwrap();
        n["anchors"] = json!([]);
        value["next_id"] = json!(2);
        value["nodes"] = json!([n]);
        let bytes = serde_json::to_vec(&value).unwrap();
        std::fs::write(&p, &bytes).unwrap();
        assert!(Document::load(&p).unwrap_err().contains("version 4"));
        assert_eq!(std::fs::read(&p).unwrap(), bytes);
    }
    let mut e = Editor::new(d.clone());
    let revision = e.revision();
    assert!(
        e.apply(|d| {
            d.nodes[0].anchors[0].outgoing = Some([f32::INFINITY, 0.0]);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(e.document, d);
    assert_eq!(e.revision(), revision);
}
fn call(server: &mut Server, name: &str, args: Value) -> (bool, Value) {
    let response=server.handle_line(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}).to_string()).unwrap();
    let r = &response["result"];
    let text = r["content"][0]["text"].as_str().unwrap();
    (
        r["isError"] == true,
        serde_json::from_str(text).unwrap_or(json!(text)),
    )
}
#[test]
fn mcp_curves_are_canonical_atomic_and_upgrade_only_with_consent() {
    let mut server = Server::default();
    let (_, s) = call(&mut server, "document_new", json!({}));
    let id = s["document_id"].clone();
    let n = curve();
    let (error, _) = call(
        &mut server,
        "document_apply",
        json!({"document_id":id,"operations":[{"op":"create_path","anchors":n.anchors,"x":n.x,"y":n.y,"width":n.width,"height":n.height,"stroke":"#ff8800","fill":"#00000000"}]}),
    );
    assert!(!error);
    let (_, state) = call(&mut server, "document_inspect", json!({"document_id":id}));
    assert_eq!(state["version"], 4);
    assert_eq!(
        serde_json::from_value::<Vec<Anchor>>(state["layers"][0]["anchors"].clone()).unwrap(),
        n.anchors
    );
    let (error, _) = call(
        &mut server,
        "document_apply",
        json!({"document_id":id,"operations":[{"op":"update_layer","id":1,"x":20},{"op":"update_layer","id":1,"anchors":[{"position":[0,0],"secret":true},{"position":[1,1]}]}]}),
    );
    assert!(error);
    let (_, after) = call(&mut server, "document_inspect", json!({"document_id":id}));
    assert_eq!(after, state);
    call(&mut server, "document_undo", json!({"document_id":id}));
    let (error, _) = call(
        &mut server,
        "document_apply",
        json!({"document_id":id,"operations":[{"op":"create_path","anchors":n.anchors,"points":[[0,0],[1,1]]}]}),
    );
    assert!(error);
    assert!(call(&mut server,"document_apply",json!({"document_id":id,"operations":[{"op":"create_path","points":[[0,0],[1,1]],"svg_path":"M 0 0 L 1 1"}]})).0);
    call(&mut server, "document_redo", json!({"document_id":id}));
    let (_, after) = call(&mut server, "document_inspect", json!({"document_id":id}));
    assert_eq!(after, state);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("old.papercraft");
    Document::default().save(&p).unwrap();
    let bytes = std::fs::read(&p).unwrap();
    let (_, s) = call(&mut server, "document_open", json!({"path":p}));
    let id = s["document_id"].clone();
    call(
        &mut server,
        "document_apply",
        json!({"document_id":id,"operations":[{"op":"create_path","anchors":n.anchors}]}),
    );
    assert!(call(&mut server, "document_save", json!({"document_id":id})).0);
    assert_eq!(std::fs::read(&p).unwrap(), bytes);
    assert!(
        !call(
            &mut server,
            "document_save",
            json!({"document_id":id,"allow_upgrade":true})
        )
        .0
    );
    assert_eq!(Document::load(&p).unwrap().required_version(), 4);
}
#[test]
fn explicit_v4_zero_stroke_and_concave_pixels_match_semantic_picking() {
    let mut n = curve();
    n.stroke = None;
    n.stroke_width = 0.0;
    let mut d = Document {
        nodes: vec![n],
        next_id: 2,
        ..Document::default()
    };
    d.normalize();
    assert_eq!(d.hit_test([-80.0, 40.0]), None);
    assert!(!d.export_svg().unwrap().contains("stroke-width=\"2\""));
    d.nodes[0] = Node {
        kind: NodeKind::Path,
        points: vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.3],
            [0.3, 0.3],
            [0.3, 1.0],
            [0.0, 1.0],
        ],
        closed: true,
        fill: [250, 10, 10, 255],
        ..Node::default()
    };
    d.normalize();
    let png = papercraft::render::export_png_region(&d, [0.0, 0.0, 100.0, 100.0], 1.0).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    assert!(img.get_pixel(10, 60)[0] > 240);
    assert_ne!(img.get_pixel(60, 60)[0], 250);
    assert_eq!(d.hit_test([10.0, 60.0]), Some(1));
    assert_eq!(d.hit_test([60.0, 60.0]), None);
}
#[test]
fn trailing_lines_cannot_bypass_the_curve_vertex_budget() {
    for scale in [0.01, 0.025, 0.05, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.7, 1.0] {
        let a = Anchor {
            position: [0.0, 0.0],
            incoming: Some([-10_000.0 * scale, 100_000.0 * scale]),
            outgoing: Some([10_000.0 * scale, 100_000.0 * scale]),
            mode: PointMode::Corner,
        };
        let mut n = Node {
            kind: NodeKind::Path,
            width: 1.0,
            height: 1.0,
            anchors: vec![a.clone(), a.clone()],
            ..Node::default()
        };
        let per = vector::flatten(&n).unwrap().len() - 1;
        let curves = (vector::MAX_FLAT_POINTS - 1) / per;
        if curves + 2 >= vector::MAX_ANCHORS {
            continue;
        }
        let lines = vector::MAX_ANCHORS - curves - 1;
        if curves * per + 1 + lines <= vector::MAX_FLAT_POINTS {
            continue;
        }
        n.anchors = vec![a; curves + 1];
        n.anchors.last_mut().unwrap().outgoing = None;
        n.anchors
            .extend((1..=lines).map(|i| Anchor::corner(pos2(i as f32, 0.0))));
        assert!(
            vector::flatten(&n).is_err(),
            "{curves} curves + {lines} straight segments exceeded the shared budget"
        );
        return;
    }
    panic!("Fixture must exercise the mixed segment budget boundary");
}
