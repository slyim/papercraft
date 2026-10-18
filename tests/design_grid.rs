//! The shell's layout values sit on a 4-point grid (see `ui.rs`). This scans the UI
//! sources for literal sizes, gaps and insets that break it. Hairlines (1) are allowed;
//! array indices and multipliers such as `2.0 * ROW` are not sizes and are skipped.

const SOURCES: [(&str, &str); 3] = [
    ("src/ui.rs", include_str!("../src/ui.rs")),
    ("src/app.rs", include_str!("../src/app.rs")),
    (
        "src/app/dashboard.rs",
        include_str!("../src/app/dashboard.rs"),
    ),
];

/// Calls whose numeric arguments are layout sizes in points.
const CALLS: [&str; 14] = [
    "add_space(",
    "vec2(",
    "Margin::same(",
    "Margin::symmetric(",
    ".size(",
    "proportional(",
    "icon_font(",
    "ui::font(",
    "exact_size(",
    "default_size(",
    ".indent(",
    "set_min_width(",
    "set_max_width(",
    "min_size(",
];

/// Margin fields written as `left: 12,`.
const FIELDS: [&str; 4] = ["left: ", "right: ", "top: ", "bottom: "];

fn literal_numbers(args: &str) -> Vec<f32> {
    let bytes = args.as_bytes();
    let mut numbers = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let starts = bytes[i].is_ascii_digit()
            && (i == 0
                || !(bytes[i - 1].is_ascii_alphanumeric() || b"_.[".contains(&bytes[i - 1])));
        if !starts {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_')
        {
            i += 1;
        }
        let rest = args[i..].trim_start();
        let before = args[..start].trim_end();
        let factor = rest.starts_with('*') || before.ends_with('*') || before.ends_with('/');
        if !factor && let Ok(value) = args[start..i].replace('_', "").parse() {
            numbers.push(value);
        }
    }
    numbers
}

fn on_grid(value: f32) -> bool {
    value == 0.0 || value == 1.0 || value % 4.0 == 0.0
}

#[test]
fn layout_literals_follow_the_four_point_grid() {
    let mut violations = Vec::new();
    for (path, source) in SOURCES {
        for (number, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for call in CALLS {
                for (index, _) in code.match_indices(call) {
                    let args = &code[index + call.len()..];
                    let args = args.split(')').next().unwrap_or_default();
                    if literal_numbers(args).into_iter().any(|v| !on_grid(v)) {
                        violations.push(format!("{path}:{}: {}", number + 1, line.trim()));
                    }
                }
            }
            for field in FIELDS {
                if let Some(rest) = code.trim_start().strip_prefix(field)
                    && let Ok(value) = rest.trim_end_matches(',').trim().parse::<f32>()
                    && !on_grid(value)
                {
                    violations.push(format!("{path}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Off-grid layout values:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_scanner_finds_sizes_and_skips_factors_and_indices() {
    assert_eq!(literal_numbers("10.0, 8.0"), [10.0, 8.0]);
    assert_eq!(literal_numbers("2.0 * ROW, ROW"), Vec::<f32>::new());
    assert_eq!(literal_numbers("frame[2], frame[3]"), Vec::<f32>::new());
    assert_eq!(literal_numbers("width / 2.0"), Vec::<f32>::new());
    assert!(!on_grid(10.0) && !on_grid(13.5) && on_grid(12.0) && on_grid(1.0));
}
