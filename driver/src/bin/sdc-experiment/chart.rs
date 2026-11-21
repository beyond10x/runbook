//! `docs/img/progress.svg`: one column per ledger entry, one row per model setting. A cell shows
//! one dot per ended run (filled: a clean pass; a ring: no clean pass, so the two differ by shape
//! and not by colour alone), the clean passes of the ended runs as a count, and the median cost
//! and wall time of its clean passes. No line joins the cells and no rate is drawn: with one to
//! five runs per cell a line or a share would claim more than the runs show. Fewer than three
//! ended runs is marked `n<3`.
//!
//! Which columns are shown is decided by the caller (`progress.rs`, `chart_model`): at most
//! `MAX_COLS`; from three releases on, the pre-release trees are one column, and only the newest
//! releases that fit are drawn, the rest being in `docs/progress.md`.
//!
//! Plain SVG with its own style: real text in generic font families, the colours of the light and
//! the dark surface switched by `prefers-color-scheme`, the same bytes for the same inputs.

use std::fmt::Write as _;

pub struct Col {
    pub label: String,
    /// What changed, in a few words.
    pub note: String,
    pub release: bool,
    /// The third header line: the experiments, or a count of them.
    pub source: String,
    pub measured: bool,
    pub ended: usize,
    pub clean: usize,
    pub open: usize,
}

pub struct CellM {
    /// One entry per ended run: whether it is a clean pass.
    pub runs: Vec<bool>,
    pub open: usize,
    pub cost: Option<f64>,
    pub wall_s: Option<f64>,
    pub experiments: Vec<String>,
}

pub struct Row {
    pub label: String,
    pub note: String,
    pub cells: Vec<Option<CellM>>,
}

pub struct Model {
    pub title: String,
    pub subtitle: String,
    pub cols: Vec<Col>,
    pub rows: Vec<Row>,
    /// Releases not drawn (the oldest), and pre-release trees folded into one column.
    pub hidden_releases: usize,
}

/// The most columns the image draws.
pub const MAX_COLS: usize = 6;
const W: f64 = 800.0;
const PAD: f64 = 24.0;
const LABEL_W: f64 = 196.0;
const DOT_STEP: f64 = 13.0;
const SMALL_N: usize = 3;

const STYLE: &str = "<style>\
text{font-family:system-ui,sans-serif}\
.bg{fill:#fcfcfb}.t1{fill:#0b0b0b}.t2{fill:#52514e}.gr{stroke:#e1e0d9}.ax{stroke:#c3c2b7}\
.ok{fill:#0ca30c}.no{fill:#fcfcfb;stroke:#898781}\
@media (prefers-color-scheme:dark){.bg{fill:#1a1a19}.t1{fill:#f0efec}.t2{fill:#c3c2b7}.gr{stroke:#2c2c2a}\
.ax{stroke:#454542}.no{fill:#1a1a19;stroke:#a3a29b}}\
</style>";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn n(x: f64) -> String {
    let s = format!("{x:.1}");
    s.strip_suffix(".0").map(String::from).unwrap_or(s)
}

fn minutes(s: f64) -> String {
    let m = s / 60.0;
    if m < 10.0 { format!("{m:.1} min") } else { format!("{m:.0} min") }
}

/// `s` cut to what fits in `width` at `size` (an estimate of the average glyph width of a
/// sans-serif face, generous so that wider faces fit too), with an ellipsis when cut.
fn fit(s: &str, width: f64, size: f64) -> String {
    let max = (width / (size * 0.55)).floor() as usize;
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let mut out: String = chars[..max.saturating_sub(1)].iter().collect();
    out = out.trim_end_matches([' ', ',', ';', ':']).to_string();
    out.push('…');
    out
}

/// `s` in at most three lines that fit `width`, broken at spaces; the last is cut by `fit`. A
/// `short` of at most 28 characters takes two lines at five columns and three at six.
fn wrap3(s: &str, width: f64, size: f64) -> Vec<String> {
    let max = ((width / (size * 0.5)).floor() as usize).max(1);
    let mut out: Vec<String> = vec![];
    let mut rest = s.trim();
    while out.len() < 2 && rest.chars().count() > max {
        let cut = rest.match_indices(' ').map(|(i, _)| i).take_while(|i| rest[..*i].chars().count() <= max).last();
        let Some(i) = cut else { break };
        out.push(rest[..i].to_string());
        rest = rest[i + 1..].trim_start();
    }
    out.push(if rest.chars().count() <= max { rest.to_string() } else { fit(rest, width, size) });
    out
}

#[allow(clippy::too_many_arguments)]
fn text(svg: &mut String, x: f64, y: f64, class: &str, size: f64, weight: u32, anchor: &str, body: &str) {
    let w = if weight >= 600 { " font-weight=\"600\"" } else { "" };
    let a = if anchor == "start" { String::new() } else { format!(" text-anchor=\"{anchor}\"") };
    let _ = writeln!(svg, "<text x=\"{}\" y=\"{}\" class=\"{class}\" font-size=\"{}\"{w}{a}>{body}</text>", n(x), n(y), n(size));
}

fn line(svg: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, class: &str) {
    let _ = writeln!(svg, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"{class}\" stroke-width=\"1\"/>", n(x1), n(y1), n(x2), n(y2));
}

fn dot(svg: &mut String, x: f64, y: f64, clean: bool) {
    if clean {
        let _ = writeln!(svg, "<circle cx=\"{}\" cy=\"{}\" r=\"4.5\" class=\"ok\"/>", n(x), n(y));
    } else {
        let _ = writeln!(svg, "<circle cx=\"{}\" cy=\"{}\" r=\"3.75\" class=\"no\" stroke-width=\"1.5\"/>", n(x), n(y));
    }
}

fn of(clean: usize, ended: usize) -> String {
    format!("{clean} of {ended}")
}

fn tip(svg: &mut String, x: f64, y: f64, w: f64, h: f64, t: &str) {
    let _ = writeln!(svg, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"transparent\"><title>{}</title></rect>", n(x), n(y), n(w), n(h), esc(t));
}

pub fn render(m: &Model) -> String {
    let cols = &m.cols;
    let avail = W - PAD - (PAD + LABEL_W);
    let cw = avail / cols.len().max(1) as f64;
    let inner = cw - 16.0;
    let x0 = PAD + LABEL_W;
    // Under 110 px a cost and a time do not fit on one line of a cell.
    let narrow = cw < 110.0;
    let per_line = (((cw - 20.0) / DOT_STEP).floor() as usize).max(1);
    let col_x = |i: usize| x0 + cw * i as f64;

    let mut body = String::new();
    // Title, subtitle, legend.
    text(&mut body, PAD, 34.0, "t1", 18.0, 600, "start", &esc(&m.title));
    text(&mut body, PAD, 56.0, "t2", 12.0, 400, "start", &esc(&m.subtitle));
    let ly = 80.0;
    dot(&mut body, PAD + 5.0, ly - 4.0, true);
    text(&mut body, PAD + 14.0, ly, "t2", 12.0, 400, "start", "clean pass");
    dot(&mut body, PAD + 94.0, ly - 4.0, false);
    text(&mut body, PAD + 103.0, ly, "t2", 12.0, 400, "start", "ended without a clean pass");
    text(&mut body, PAD + 272.0, ly, "t1", 12.0, 600, "start", "1 of 2");
    text(&mut body, PAD + 312.0, ly, "t2", 12.0, 400, "start", "clean passes of ended runs");
    text(&mut body, PAD + 474.0, ly, "t1", 12.0, 600, "start", "n&lt;3");
    text(&mut body, PAD + 502.0, ly, "t2", 12.0, 400, "start", "fewer than 3 ended runs");

    // Group headers: pre-release trees, then releases.
    let gy = 112.0;
    let mut i = 0;
    while i < cols.len() {
        let rel = cols[i].release;
        let mut j = i;
        while j + 1 < cols.len() && cols[j + 1].release == rel {
            j += 1;
        }
        let width = col_x(j + 1) - col_x(i) - 16.0;
        let name = match (rel, j + 1 - i) {
            (true, k) if m.hidden_releases > 0 => format!("Releases, newest {k} of {}", k + m.hidden_releases),
            (true, _) => "Releases".to_string(),
            (false, 1) => "Pre-release".to_string(),
            (false, _) => "Pre-release trees (not releases)".to_string(),
        };
        text(&mut body, col_x(i) + 8.0, gy, "t2", 11.0, 600, "start", &esc(&fit(&name, width, 11.0)));
        line(&mut body, col_x(i) + 8.0, gy + 6.0, col_x(j + 1) - 8.0, gy + 6.0, "ax");
        i = j + 1;
    }
    // Column headers.
    for (i, c) in cols.iter().enumerate() {
        let x = col_x(i) + 8.0;
        text(&mut body, x, 140.0, "t1", 14.0, 600, "start", &esc(&fit(&c.label, inner, 14.0)));
        for (k, l) in wrap3(&c.note, inner, 11.0).iter().enumerate() {
            text(&mut body, x, 156.0 + 12.5 * k as f64, "t2", 11.0, 400, "start", &esc(l));
        }
        text(&mut body, x, 196.0, "t2", 10.0, 400, "start", &esc(&fit(&c.source, inner, 10.0)));
    }
    let header_bottom = 206.0;
    line(&mut body, PAD, header_bottom, W - PAD, header_bottom, "ax");

    // Totals row.
    let ty = header_bottom;
    text(&mut body, PAD, ty + 21.0, "t1", 13.0, 600, "start", "All settings");
    text(&mut body, PAD, ty + 36.0, "t2", 11.0, 400, "start", "clean passes of ended runs");
    for (i, c) in cols.iter().enumerate() {
        let x = col_x(i) + 8.0;
        let right = col_x(i + 1) - 8.0;
        let t = if !c.measured {
            text(&mut body, x, ty + 27.0, "t2", 12.0, 400, "start", "no runs yet");
            format!("{}: no run on this tree yet", c.label)
        } else {
            text(&mut body, x, ty + 27.0, "t1", 17.0, 600, "start", &of(c.clean, c.ended));
            if c.ended < SMALL_N {
                text(&mut body, right, ty + 27.0, "t2", 10.5, 400, "end", "n&lt;3");
            }
            if c.open > 0 {
                text(&mut body, x, ty + 41.0, "t2", 10.5, 400, "start", &format!("+{} running", c.open));
            }
            format!("{}: {} clean passes of {} ended runs, {} not ended ({})", c.label, c.clean, c.ended, c.open, c.source)
        };
        tip(&mut body, col_x(i), ty, cw, 48.0, &t);
    }
    let mut y = ty + 48.0;
    line(&mut body, PAD, y, W - PAD, y, "gr");

    // Setting rows.
    for r in &m.rows {
        let lines = r.cells.iter().flatten().map(|c| c.runs.len().div_ceil(per_line).max(1)).max().unwrap_or(1);
        let extra = (lines - 1) as f64 * DOT_STEP;
        let open_line = r.cells.iter().flatten().any(|c| c.open > 0 && !c.runs.is_empty());
        let two = narrow && r.cells.iter().flatten().any(|c| c.cost.is_some());
        let h = 66.0 + extra + if open_line { 14.0 } else { 0.0 } + if two { 14.0 } else { 0.0 };
        text(&mut body, PAD, y + 22.0, "t1", 13.0, 400, "start", &esc(&fit(&r.label, LABEL_W - 8.0, 13.0)));
        text(&mut body, PAD, y + 37.0, "t2", 11.0, 400, "start", &esc(&fit(&r.note, LABEL_W - 8.0, 11.0)));
        for (i, cell) in r.cells.iter().enumerate() {
            let Some(c) = cell else { continue };
            let x = col_x(i) + 8.0;
            let right = col_x(i + 1) - 8.0;
            for (k, clean) in c.runs.iter().enumerate() {
                dot(&mut body, x + 5.0 + (k % per_line) as f64 * DOT_STEP, y + 16.0 + (k / per_line) as f64 * DOT_STEP, *clean);
            }
            let clean = c.runs.iter().filter(|x| **x).count();
            let ended = c.runs.len();
            let cy = y + 38.0 + extra;
            if ended == 0 {
                text(&mut body, x, cy, "t2", 12.0, 400, "start", &format!("{} running", c.open));
                text(&mut body, x, cy + 15.0, "t2", 11.0, 400, "start", "not counted yet");
            } else {
                text(&mut body, x, cy, "t1", 12.5, 600, "start", &of(clean, ended));
                if ended < SMALL_N {
                    text(&mut body, right, cy, "t2", 10.5, 400, "end", "n&lt;3");
                }
                let money: Vec<String> = match (c.cost, c.wall_s) {
                    (Some(cost), Some(w)) if narrow => vec![format!("${cost:.2}"), minutes(w)],
                    (Some(cost), Some(w)) => vec![format!("${cost:.2} · {}", minutes(w))],
                    (Some(cost), None) => vec![format!("${cost:.2}")],
                    _ => vec!["no clean pass".to_string()],
                };
                for (k, l) in money.iter().enumerate() {
                    text(&mut body, x, cy + 15.0 + 14.0 * k as f64, "t2", 11.0, 400, "start", &esc(&fit(l, inner, 11.0)));
                }
                if c.open > 0 {
                    text(&mut body, x, cy + 15.0 + 14.0 * money.len() as f64, "t2", 10.5, 400, "start", &format!("+{} running", c.open));
                }
            }
            let mut t = format!("{}, {}: {} clean passes of {} ended runs", cols[i].label, r.label, clean, ended);
            if c.open > 0 {
                let _ = write!(t, ", {} not ended", c.open);
            }
            if let (Some(cost), Some(w)) = (c.cost, c.wall_s) {
                let _ = write!(t, "; median clean pass {cost:.2} USD, {}", minutes(w));
            }
            let _ = write!(t, " ({})", c.experiments.join(", "));
            tip(&mut body, col_x(i), y, cw, h, &t);
        }
        y += h;
        line(&mut body, PAD, y, W - PAD, y, "gr");
    }
    // Separator between pre-release trees and releases.
    if let Some(k) = cols.iter().position(|c| c.release).filter(|k| *k > 0) {
        line(&mut body, col_x(k), 102.0, col_x(k), y, "ax");
    }
    // Footer.
    let mut fy = y + 22.0;
    let mut foot = vec![
        "One dot per ended run. Under the count: the median cost in US dollars (model and decider calls) and the median wall".to_string(),
        "time in minutes (one run's elapsed time, first to last event) of the cell's clean passes. Every figure with its".to_string(),
        "experiment record: docs/progress.md and docs/progress.json, written by sdc-experiment progress from experiments/.".to_string(),
    ];
    if m.hidden_releases > 0 {
        foot.push(format!("The {} older releases are in docs/progress.md.", m.hidden_releases));
    }
    for f in &foot {
        text(&mut body, PAD, fy, "t2", 11.0, 400, "start", &esc(f));
        fy += 15.0;
    }
    let height = fy + 6.0;

    let desc = {
        let mut d = String::from("Clean passes of ended runs per column: ");
        let parts: Vec<String> = cols.iter().map(|c| {
            let kind = if c.release { "release" } else { "pre-release" };
            if c.measured { format!("{} ({kind}, {}) {}", c.label, c.note, of(c.clean, c.ended)) } else { format!("{} ({kind}, {}) no runs yet", c.label, c.note) }
        }).collect();
        d.push_str(&parts.join("; "));
        d.push_str(". Rows give the same per model setting with the median cost and wall time of a clean pass.");
        d
    };
    let mut svg = String::new();
    let _ = writeln!(svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\" role=\"img\" aria-labelledby=\"title desc\">",
        n(W), n(height), n(W), n(height));
    let _ = writeln!(svg, "<title id=\"title\">{}</title>", esc(&m.title));
    let _ = writeln!(svg, "<desc id=\"desc\">{}</desc>", esc(&desc));
    svg.push_str(STYLE);
    svg.push('\n');
    let _ = writeln!(svg, "<rect width=\"{}\" height=\"{}\" class=\"bg\"/>", n(W), n(height));
    svg.push_str(&body);
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_label_is_cut_with_an_ellipsis() {
        assert_eq!(fit("short", 100.0, 10.0), "short");
        let cut = fit("hermetic claude-cli adapter; every node class has an explicit default effort", 90.0, 11.0);
        assert!(cut.ends_with('…') && cut.chars().count() <= 15, "{cut}");
    }

    #[test]
    fn a_short_of_28_characters_is_never_cut_at_six_columns() {
        let cw = (W - PAD - (PAD + LABEL_W)) / MAX_COLS as f64;
        for s in ["hermetic decider, effort set", "abcdefghijkl abcdefghijkl ab", "a b c d e f g h i j k l m n"] {
            let lines = wrap3(s, cw - 16.0, 11.0);
            assert!(lines.len() <= 3 && lines.iter().all(|l| !l.contains('…')), "{s}: {lines:?}");
            assert_eq!(lines.join(" "), s);
        }
    }
}
