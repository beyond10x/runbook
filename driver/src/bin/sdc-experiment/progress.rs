//! The progress ledger (docs/improving.md). `sdc-experiment progress` reads the release table
//! `experiments/releases.yaml`, the experiment records it names and their committed run summaries,
//! and writes `docs/progress.md`, `docs/progress.json` and `docs/img/progress.svg`. Nothing in the
//! three files is written by hand; `check --all` fails when one of them is stale.
//!
//! An entry of the table is a release or a pre-release tree: its date, its tree, the changes it
//! holds (commit, one line, and whether the change claims a measured gain, is not measured yet, or
//! affects no run metric) and the experiment records whose runs measured that tree. Every run of
//! those records is put in one model setting of the table by its settings; a run counts in one
//! entry only. A clean pass is the records' pass rule: closed, delivered, no reopen, forced step or
//! unstick.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value as J};
use serde_yaml::Value as Y;

use super::chart;
use super::{
    check_record, clean_pass, compute_result, has_abs_path, is_hex, load_variants, one_line, read_record, record_paths,
    settings_map, stat, yscalar, yseq, ystr, Run, Stat,
};

/// The release table, beside the records.
pub(super) const RELEASES: &str = "releases.yaml";
const KINDS: [&str; 2] = ["pre-release", "release"];
const SMALL_N: usize = 3;
/// The longest `short`: two lines of a column header at 800 px.
const SHORT_MAX: usize = 28;
/// The longest word of a `short`: one line of the narrowest column.
const SHORT_WORD_MAX: usize = 12;

pub(super) struct Setting {
    pub id: String,
    pub label: String,
    pub note: String,
    matches: BTreeMap<String, String>,
}

struct Change {
    commit: String,
    line: String,
    effect: String,
    experiment: Option<String>,
}

struct Entry {
    id: String,
    kind: String,
    version: Option<String>,
    label: String,
    short: String,
    date: String,
    tree: Option<String>,
    tree_note: String,
    experiments: Vec<String>,
    changes: Vec<Change>,
    limits: Vec<String>,
}

struct Ledger {
    input_name: String,
    input_description: String,
    input_sha: String,
    settings: Vec<Setting>,
    entries: Vec<Entry>,
    limits: Vec<String>,
}

fn text_of(v: &Y, k: &str) -> String {
    v.get(k).filter(|x| !x.is_null()).map(yscalar).unwrap_or_default()
}

fn lines_of(v: &Y, k: &str) -> Vec<String> {
    yseq(v, k).iter().map(yscalar).collect()
}

fn is_date(s: &str) -> bool {
    s.len() == 10 && s.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() })
}

fn is_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// The table and the problems of its fields, each a sentence naming the field.
fn parse(y: &Y, raw: &str) -> (Ledger, Vec<String>) {
    let mut p = vec![];
    if ystr(y, "format") != Some("sdc.releases/1") {
        p.push("`format` must be sdc.releases/1".to_string());
    }
    if has_abs_path(raw) {
        p.push("the table holds an absolute path".into());
    }
    let input = y.get("input").cloned().unwrap_or(Y::Null);
    let (input_name, input_sha) = (text_of(&input, "name"), text_of(&input, "sha256"));
    let input_description = text_of(&input, "description");
    if input_description.contains('\n') {
        p.push("`input.description` is one line".into());
    }
    if input_name.is_empty() || !is_hex(&input_sha, 64, 64) {
        p.push("`input` needs `name` and `sha256`, the input every counted run used".into());
    }
    let mut settings: Vec<Setting> = vec![];
    for s in yseq(y, "settings") {
        let st = Setting { id: text_of(&s, "id"), label: text_of(&s, "label"), note: text_of(&s, "note"), matches: settings_map(s.get("match")) };
        if st.id.is_empty() || settings.iter().any(|x| x.id == st.id) {
            p.push(format!("a setting needs a unique `id` ({:?})", st.id));
        }
        if !one_line(&st.label) || st.note.contains('\n') {
            p.push(format!("setting {:?}: `label` must be one line, `note` at most one", st.id));
        }
        if st.matches.is_empty() {
            p.push(format!("setting {:?}: `match` needs the settings keys and values that make a run this setting", st.id));
        }
        settings.push(st);
    }
    if settings.is_empty() {
        p.push("`settings` needs at least one model setting".into());
    }
    let limits = lines_of(y, "limits");
    let mut entries: Vec<Entry> = vec![];
    let mut experiments_seen = BTreeSet::new();
    for e in yseq(y, "entries") {
        let id = text_of(&e, "id");
        let changes: Vec<Change> = yseq(&e, "changes").iter().map(|c| Change {
            commit: text_of(c, "commit"),
            line: text_of(c, "line"),
            effect: text_of(c, "effect"),
            experiment: c.get("experiment").filter(|x| !x.is_null()).map(yscalar),
        }).collect();
        let en = Entry {
            kind: text_of(&e, "kind"),
            version: e.get("version").filter(|x| !x.is_null()).map(yscalar),
            label: Some(text_of(&e, "label")).filter(|l| !l.is_empty()).unwrap_or_else(|| id.clone()),
            short: text_of(&e, "short"),
            date: text_of(&e, "date"),
            tree: e.get("tree").filter(|x| !x.is_null()).map(yscalar),
            tree_note: text_of(&e, "tree_note"),
            experiments: lines_of(&e, "experiments"),
            changes,
            limits: lines_of(&e, "limits"),
            id,
        };
        let at = format!("entry {:?}", en.id);
        if en.id.is_empty() || entries.iter().any(|x| x.id == en.id) {
            p.push(format!("{at}: an entry needs a unique `id`"));
        }
        if !KINDS.contains(&en.kind.as_str()) {
            p.push(format!("{at}: `kind` must be pre-release or release"));
        }
        match (en.kind.as_str(), &en.version) {
            ("release", Some(v)) if is_version(v) && *v == en.id => {}
            ("release", _) => p.push(format!("{at}: a release needs `version` (x.y.z), the same as its `id`")),
            (_, Some(_)) => p.push(format!("{at}: only a release has a `version`")),
            _ => {}
        }
        if !is_date(&en.date) {
            p.push(format!("{at}: `date` must be YYYY-MM-DD"));
        }
        match en.tree.as_deref() {
            None if en.kind == "release" && one_line(&en.tree_note) => {}
            None => p.push(format!("{at}: `tree` may be null only for a release not tagged yet, with `tree_note` saying where it is prepared")),
            Some("several") if en.kind == "pre-release" && one_line(&en.tree_note) => {}
            Some("several") => p.push(format!("{at}: `tree: several` is for pre-release trees only, with `tree_note` naming them")),
            Some(t) if is_hex(t, 7, 40) => {}
            Some(_) => p.push(format!("{at}: `tree` must be a commit, `several` or null")),
        }
        if !one_line(&en.label) || en.tree_note.contains('\n') {
            p.push(format!("{at}: `label` and `tree_note` are one line each"));
        }
        if !one_line(&en.short) || en.short.chars().count() > SHORT_MAX || en.short.split(' ').any(|w| w.chars().count() > SHORT_WORD_MAX) {
            p.push(format!("{at}: `short` must say what changed in at most {SHORT_MAX} characters on one line, no word over {SHORT_WORD_MAX} (the image's column header)"));
        }
        for x in &en.experiments {
            if !experiments_seen.insert(x.clone()) {
                p.push(format!("{at}: experiment {x} is listed in two entries"));
            }
        }
        if en.changes.is_empty() {
            p.push(format!("{at}: `changes` needs at least one change"));
        }
        for c in &en.changes {
            if !is_hex(&c.commit, 7, 40) {
                p.push(format!("{at}: change {:?}: `commit` must be a commit", c.commit));
            }
            if !one_line(&c.line) {
                p.push(format!("{at}: change {}: `line` must say the change on one line", c.commit));
            }
            match (c.effect.as_str(), &c.experiment) {
                ("measured", Some(_)) | ("unmeasured" | "none", None) => {}
                ("measured", None) => p.push(format!("{at}: change {}: a measured change names its `experiment`", c.commit)),
                ("unmeasured" | "none", Some(_)) => p.push(format!("{at}: change {}: only a measured change names an `experiment`", c.commit)),
                _ => p.push(format!("{at}: change {}: `effect` must be measured, unmeasured or none", c.commit)),
            }
        }
        if en.limits.iter().chain(&limits).any(|l| !one_line(l)) {
            p.push(format!("{at}: every limit is one line"));
        }
        entries.push(en);
    }
    if entries.is_empty() {
        p.push("`entries` needs at least one entry".into());
    }
    (Ledger { input_name, input_description, input_sha, settings, entries, limits }, p)
}

// ---- figures -----------------------------------------------------------------------------------

struct RunFig {
    name: String,
    clean: bool,
    cost: Option<f64>,
    wall: Option<f64>,
    end: String,
}

/// The runs of one variant of one record that fall in one setting.
struct Cell {
    setting: usize,
    experiment: String,
    variant: String,
    runs: Vec<RunFig>,
    /// Listed runs with no summary of an ended run: still open, not counted.
    open: Vec<String>,
}

fn clean_stat(runs: &[&RunFig], f: fn(&RunFig) -> Option<f64>) -> Option<Stat> {
    stat(&runs.iter().filter(|r| r.clean).filter_map(|r| f(r)).collect::<Vec<_>>())
}

struct Counts {
    ended: usize,
    clean: usize,
    open: usize,
}

fn counts<'a>(cells: impl Iterator<Item = &'a Cell>) -> Counts {
    let mut c = Counts { ended: 0, clean: 0, open: 0 };
    for x in cells {
        c.ended += x.runs.len();
        c.clean += x.runs.iter().filter(|r| r.clean).count();
        c.open += x.open.len();
    }
    c
}

struct ExpFig {
    id: String,
    kind: String,
    purpose: String,
    tree: String,
    trees: Vec<String>,
    hypothesis: Option<String>,
    metric: Option<String>,
    direction: Option<String>,
    min_effect: Option<f64>,
    verdict: String,
    why: String,
    /// Per variant: id, listed, ended, clean passes, primary metric.
    variants: Vec<(String, usize, usize, usize, Option<Stat>)>,
    confounds: Vec<(String, String)>,
}

fn stat_of_json(v: &J) -> Option<Stat> {
    let n = v["n"].as_u64()? as usize;
    (n > 0).then(|| Stat { n, median: v["median"].as_f64().unwrap_or(0.0), min: v["min"].as_f64().unwrap_or(0.0), max: v["max"].as_f64().unwrap_or(0.0) })
}

fn exp_fig(r: &Y, result: &J) -> ExpFig {
    let hypothesis = ystr(r, "hypothesis").filter(|h| !h.trim().is_empty()).map(String::from);
    let primary = r.get("primary").cloned().unwrap_or(Y::Null);
    let variants = result["variants"].as_array().into_iter().flatten().map(|v| (
        v["id"].as_str().unwrap_or("").to_string(),
        v["listed"].as_u64().unwrap_or(0) as usize,
        v["ended"].as_u64().unwrap_or(0) as usize,
        v["clean_passes"].as_u64().unwrap_or(0) as usize,
        v.get("primary").and_then(stat_of_json),
    )).collect();
    ExpFig {
        id: ystr(r, "id").unwrap_or("").to_string(),
        kind: ystr(r, "kind").unwrap_or("").to_string(),
        purpose: ystr(r, "purpose").unwrap_or("").to_string(),
        tree: r.get("tree_commit").filter(|t| !t.is_null()).map(yscalar).map(|t| if is_hex(&t, 7, 40) { t[..7].to_string() } else { t }).unwrap_or_default(),
        trees: lines_of(r, "trees"),
        metric: hypothesis.as_ref().and_then(|_| ystr(&primary, "metric").map(String::from)),
        direction: hypothesis.as_ref().and_then(|_| ystr(&primary, "direction").map(String::from)),
        min_effect: hypothesis.as_ref().and_then(|_| primary.get("min_effect").and_then(Y::as_f64)),
        hypothesis,
        verdict: if ystr(r, "status") == Some("planned") { "planned, no run yet".to_string() } else { result["verdict"].as_str().unwrap_or("").to_string() },
        why: result["why"].as_str().unwrap_or("").to_string(),
        variants,
        confounds: yseq(r, "known_confounds").iter().map(|c| (text_of(c, "key"), text_of(c, "note"))).collect(),
    }
}

fn run_fig(r: &Run) -> RunFig {
    let m = &r.doc["metrics"];
    RunFig {
        name: r.name.clone(),
        clean: clean_pass(r),
        cost: m["cost_usd"].as_f64(),
        wall: m["wall_s"].as_f64(),
        end: m["end"].as_str().unwrap_or("").to_string(),
    }
}

struct EntryFig {
    exps: Vec<ExpFig>,
    cells: Vec<Cell>,
}

struct Built {
    ledger: Ledger,
    figs: Vec<EntryFig>,
    others: Vec<ExpFig>,
    /// Every input file and its sha256, relative to the repository.
    sources: Vec<(String, String)>,
    problems: Vec<String>,
}

fn sha_file(path: &Path) -> String {
    super::sha256_hex(&fs::read_to_string(path).unwrap_or_default())
}

/// The repository that holds the experiments directory: the ledger's paths are relative to it.
pub(super) fn repo_of(exp: &Path) -> PathBuf {
    match exp.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// The ledger's figures, or None when the experiments directory has no release table.
fn build(exp: &Path) -> Result<Option<Built>, String> {
    let path = exp.join(RELEASES);
    if !path.is_file() {
        return Ok(None);
    }
    let (y, raw) = read_record(&path)?;
    let (ledger, mut problems) = parse(&y, &raw);
    let mut sources = vec![(format!("experiments/{RELEASES}"), super::sha256_hex(&raw))];
    let mut records: BTreeMap<String, (Y, ExpFig, Vec<Cell>)> = BTreeMap::new();
    for p in record_paths(exp)? {
        let (r, rraw) = read_record(&p)?;
        if !check_record(&r, &p, &rraw).is_empty() {
            continue; // check --all reports the record itself
        }
        let id = ystr(&r, "id").unwrap_or("").to_string();
        let loaded = load_variants(&r, exp)?;
        let result = compute_result(&r, &loaded);
        let fig = exp_fig(&r, &result);
        let mut cells = vec![];
        for (v, rs, missing) in &loaded {
            let hits: Vec<usize> = ledger.settings.iter().enumerate()
                .filter(|(_, s)| s.matches.iter().all(|(k, x)| v.settings.get(k) == Some(x)))
                .map(|(i, _)| i).collect();
            cells.push((hits, Cell { setting: 0, experiment: id.clone(), variant: v.id.clone(), runs: rs.iter().map(run_fig).collect(), open: missing.clone() }));
        }
        let mut kept = vec![];
        for (hits, mut c) in cells {
            match hits.as_slice() {
                [i] => {
                    c.setting = *i;
                    kept.push(c);
                }
                [] => kept.push(Cell { setting: usize::MAX, ..c }),
                _ => kept.push(Cell { setting: usize::MAX - 1, ..c }),
            }
        }
        sources.push((format!("experiments/{id}.yaml"), super::sha256_hex(&rraw)));
        let mut sums: Vec<PathBuf> = fs::read_dir(exp.join(&id).join("runs")).into_iter().flatten().flatten().map(|e| e.path()).collect();
        sums.sort();
        for s in sums {
            sources.push((format!("experiments/{id}/runs/{}", s.file_name().unwrap_or_default().to_string_lossy()), sha_file(&s)));
        }
        records.insert(id, (r, fig, kept));
    }
    let named: BTreeSet<&str> = ledger.entries.iter().flat_map(|e| e.experiments.iter().map(String::as_str)).collect();
    let mut seen_runs: BTreeMap<String, String> = BTreeMap::new();
    let mut figs = vec![];
    for e in &ledger.entries {
        let at = format!("{RELEASES}: entry {:?}", e.id);
        let mut fig = EntryFig { exps: vec![], cells: vec![] };
        for x in &e.experiments {
            let Some((r, ef, cells)) = records.remove(x) else {
                problems.push(format!("{at}: experiment {x} has no valid record experiments/{x}.yaml"));
                continue;
            };
            if !yseq(&r, "inputs").iter().any(|i| ystr(i, "sha256") == Some(ledger.input_sha.as_str())) {
                problems.push(format!("{at}: experiment {x} has no input with the table's sha256 ({}): its runs ran another input", ledger.input_name));
            }
            for c in cells {
                match c.setting {
                    usize::MAX => problems.push(format!("{at}: experiment {x}, variant {}: its settings match no setting of the table", c.variant)),
                    i if i == usize::MAX - 1 => problems.push(format!("{at}: experiment {x}, variant {}: its settings match more than one setting", c.variant)),
                    _ => {
                        for n in c.runs.iter().map(|r| &r.name).chain(&c.open) {
                            if let Some(prev) = seen_runs.insert(n.clone(), e.id.clone()) {
                                problems.push(format!("{at}: run {n} is counted in entry {prev:?} as well"));
                            }
                        }
                        fig.cells.push(c);
                    }
                }
            }
            fig.exps.push(ef);
        }
        for c in &e.changes {
            if let Some(x) = &c.experiment {
                if !named.contains(x.as_str()) && !records.contains_key(x) {
                    problems.push(format!("{at}: change {}: experiment {x} has no valid record", c.commit));
                }
            }
        }
        fig.cells.sort_by(|a, b| (a.setting, &a.experiment, &a.variant).cmp(&(b.setting, &b.experiment, &b.variant)));
        figs.push(fig);
    }
    let others = records.into_values().map(|(_, f, _)| f).collect();
    Ok(Some(Built { ledger, figs, others, sources, problems }))
}

// ---- docs/progress.md --------------------------------------------------------------------------

fn usd(x: f64) -> String {
    format!("{x:.2}")
}

fn secs(x: f64) -> String {
    format!("{x:.0}")
}

fn stat_md(s: &Option<Stat>, f: fn(f64) -> String) -> String {
    s.as_ref().map_or("-".into(), |s| format!("{} [{}, {}] ({})", f(s.median), f(s.min), f(s.max), s.n))
}

fn of_md(c: &Counts) -> String {
    if c.ended == 0 { "no ended run".into() } else { format!("{} of {}", c.clean, c.ended) }
}

fn rate_md(c: &Counts) -> String {
    match c.ended {
        0 => "-".into(),
        n if n < SMALL_N => format!("none given, n {n} < {SMALL_N}"),
        n => format!("{:.0} %", 100.0 * c.clean as f64 / n as f64),
    }
}

fn link(id: &str) -> String {
    format!("[`{id}`](../experiments/{id}/report.md)")
}

fn cell_md(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn effect_md(c: &Change, exps: &BTreeMap<&str, &ExpFig>) -> String {
    match c.effect.as_str() {
        "none" => "no run metric affected".into(),
        "unmeasured" => "not measured: no experiment has tested this change".into(),
        _ => {
            let x = c.experiment.as_deref().unwrap_or("");
            match exps.get(x).map(|f| f.verdict.as_str()) {
                Some("held") => format!("measured gain: {} held", link(x)),
                Some(v) => format!("gain claimed, not shown: {} is {v}", link(x)),
                None => format!("gain claimed by {}, which has no figures", link(x)),
            }
        }
    }
}

fn kind_md(e: &Entry) -> &'static str {
    if e.kind == "release" { "Release" } else { "Pre-release tree, not a release" }
}

fn tree_md(e: &Entry) -> String {
    match e.tree.as_deref() {
        None => e.tree_note.clone(),
        Some("several") => format!("several ({})", e.tree_note),
        Some(t) if e.tree_note.is_empty() => format!("`{}`", &t[..t.len().min(7)]),
        Some(t) => format!("`{}` ({})", &t[..t.len().min(7)], e.tree_note),
    }
}

fn setting_name(b: &Built, i: usize) -> &str {
    b.ledger.settings.get(i).map_or("?", |s| s.label.as_str())
}

/// The limits of one entry's measurement: the table's own lines, then what the records give.
fn entry_limits(b: &Built, e: &Entry, f: &EntryFig) -> Vec<String> {
    let mut out: Vec<String> = e.limits.clone();
    if f.exps.is_empty() {
        return out;
    }
    let mut per_setting: BTreeMap<usize, usize> = BTreeMap::new();
    for c in &f.cells {
        *per_setting.entry(c.setting).or_default() += c.runs.len();
    }
    let small: Vec<&str> = per_setting.iter().filter(|(_, n)| **n < SMALL_N).map(|(i, _)| setting_name(b, *i)).collect();
    if !small.is_empty() {
        let which = if small.len() == per_setting.len() { "every setting".to_string() } else { small.join(", ") };
        out.push(format!("Fewer than {SMALL_N} ended runs in {which}: no rate is given there and no difference between settings or trees is claimed (`docs/workflow-metrics.md` § 6)."));
    }
    for x in &f.exps {
        if x.kind == "retrospective" {
            out.push(format!("`{}` is retrospective: written after its runs, it describes them and decides nothing.", x.id));
        }
        if x.tree == "several" {
            out.push(format!("`{}` ran on {} trees: its runs are not samples of one tree.", x.id, x.trees.len()));
        }
        for (k, n) in &x.confounds {
            out.push(format!("`{}`, known confound `{k}`: {n}", x.id));
        }
    }
    let open: Vec<&str> = f.cells.iter().flat_map(|c| c.open.iter().map(String::as_str)).collect();
    if !open.is_empty() {
        out.push(format!("Listed but not ended, so not counted: {}.", open.iter().map(|o| format!("`{o}`")).collect::<Vec<_>>().join(", ")));
    }
    out
}

fn primary_md(md: &mut String, x: &ExpFig) {
    let Some(h) = &x.hypothesis else {
        let _ = writeln!(md, "- {}: no hypothesis, so no primary metric, no before and after, and verdict **{}**.", link(&x.id), x.verdict);
        return;
    };
    let _ = writeln!(md, "- {}: hypothesis \"{}\" Primary metric `{}`, expected to {} by at least {}. Verdict **{}**: {}.",
        link(&x.id), cell_md(h), x.metric.as_deref().unwrap_or(""), x.direction.as_deref().unwrap_or(""),
        super::fmt(x.min_effect.unwrap_or(0.0)), x.verdict, x.why);
    for (v, _, ended, _, p) in &x.variants {
        let _ = writeln!(md, "  - {v}: {} over {ended} ended runs", stat_md(p, super::fmt));
    }
}

fn render_md(b: &Built) -> String {
    let l = &b.ledger;
    let mut md = String::from("# Progress\n\n");
    md.push_str("Written by `sdc-experiment progress` from `experiments/releases.yaml`, the experiment records `experiments/<id>.yaml` and their run summaries `experiments/<id>/runs/*.json`; `sdc-experiment check --all` fails when this file, `docs/progress.json` or `docs/img/progress.svg` is stale. Nothing here is written by hand. The loop that adds an entry: `docs/improving.md`.\n\n");
    md.push_str("![Clean passes of ended runs per release and model setting, with the median cost and wall time of a clean pass](img/progress.svg)\n\n");
    let _ = writeln!(md, "Every figure counts runs of one input, `{}` (sha256 `{}`), started with no person (autonomy A5). A **clean pass** is a run that closed, delivered, and needed no reopen, forced step or unstick (`pass_rule: clean-delivery`, `docs/workflow-metrics.md` § 7). Counts come before rates, and a rate is given only from {SMALL_N} ended runs. Cost is `run.cost_usd` (model calls and decider calls), wall time `run.wall_s`; both are given over the clean passes as median [min, max] (n). Every figure names the experiment record it comes from. Pre-release trees are the trial trees before the first release; they are no release.\n",
        l.input_name, &l.input_sha[..l.input_sha.len().min(12)]);
    md.push_str("## Summary\n\n| entry | kind | date | tree | tested by | clean passes of ended runs | by setting |\n|---|---|---|---|---|---|---|\n");
    for (e, f) in l.entries.iter().zip(&b.figs) {
        let c = counts(f.cells.iter());
        let tested = if f.exps.is_empty() { "no experiment yet".to_string() } else { f.exps.iter().map(|x| format!("`{}`", x.id)).collect::<Vec<_>>().join(", ") };
        let mut by: BTreeMap<usize, Counts> = BTreeMap::new();
        for x in &f.cells {
            let k = by.entry(x.setting).or_insert(Counts { ended: 0, clean: 0, open: 0 });
            k.ended += x.runs.len();
            k.clean += x.runs.iter().filter(|r| r.clean).count();
        }
        let by_s = by.iter().map(|(i, k)| format!("{} {}", l.settings[*i].id, of_md(k))).collect::<Vec<_>>().join("; ");
        let tree = match e.tree.as_deref() { None => "not tagged yet".to_string(), Some("several") => "several".to_string(), Some(t) => format!("`{}`", &t[..t.len().min(7)]) };
        let _ = writeln!(md, "| [{}](#{}) | {} | {} | {tree} | {tested} | {} | {} |", e.label, anchor(e), e.kind, e.date,
            if f.exps.is_empty() { "no run".into() } else { of_md(&c) }, if by_s.is_empty() { "-".into() } else { by_s });
    }
    md.push_str("\nSettings (a run is in a setting when its record's settings have every value of `match`):\n\n| setting | label | note | match |\n|---|---|---|---|\n");
    for s in &l.settings {
        let m = s.matches.iter().map(|(k, v)| format!("`{k}` {v}")).collect::<Vec<_>>().join(", ");
        let _ = writeln!(md, "| `{}` | {} | {} | {m} |", s.id, cell_md(&s.label), cell_md(&s.note));
    }
    if !l.limits.is_empty() {
        md.push_str("\nLimits of every figure:\n\n");
        for x in &l.limits {
            let _ = writeln!(md, "- {x}");
        }
    }
    let all_exps: BTreeMap<&str, &ExpFig> = b.figs.iter().flat_map(|f| f.exps.iter()).chain(&b.others).map(|x| (x.id.as_str(), x)).collect();
    for (e, f) in l.entries.iter().zip(&b.figs) {
        let _ = writeln!(md, "\n## {}\n", e.label);
        let _ = writeln!(md, "{}, {}. Tree: {}.\n", kind_md(e), e.date, tree_md(e));
        md.push_str("What changed:\n\n| commit | change | effect on run metrics |\n|---|---|---|\n");
        for c in &e.changes {
            let _ = writeln!(md, "| `{}` | {} | {} |", c.commit, cell_md(&c.line), effect_md(c, &all_exps));
        }
        if f.exps.is_empty() {
            md.push_str("\nTested by: no experiment yet. No run used this tree, so there is no figure for it.\n");
        } else {
            md.push_str("\nTested by, primary metric and verdict:\n\n");
            for x in &f.exps {
                primary_md(&mut md, x);
            }
            let c = counts(f.cells.iter());
            let ids = f.exps.iter().map(|x| format!("`{}`", x.id)).collect::<Vec<_>>().join(", ");
            let _ = writeln!(md, "\nClean passes of ended runs, all settings: **{}** ({ids}); rate: {}.\n", of_md(&c), rate_md(&c));
            md.push_str("| setting | experiment | variant | clean passes of ended runs | rate | cost of a clean pass, USD | wall time of a clean pass, s | runs (clean passes in bold) |\n|---|---|---|---|---|---|---|---|\n");
            for x in &f.cells {
                let rs: Vec<&RunFig> = x.runs.iter().collect();
                let k = counts(std::iter::once(x));
                let names = x.runs.iter().map(|r| if r.clean { format!("**{}**", r.name) } else { format!("{} ({})", r.name, cell_md(&r.end)) })
                    .chain(x.open.iter().map(|o| format!("{o} (not ended)"))).collect::<Vec<_>>().join(", ");
                let _ = writeln!(md, "| {} | `{}` | {} | {} | {} | {} | {} | {names} |", cell_md(setting_name(b, x.setting)), x.experiment, x.variant,
                    of_md(&k), rate_md(&k), stat_md(&clean_stat(&rs, |r| r.cost), usd), stat_md(&clean_stat(&rs, |r| r.wall), secs));
            }
        }
        let limits = entry_limits(b, e, f);
        if !limits.is_empty() {
            md.push_str("\nLimits of this measurement:\n\n");
            for x in limits {
                let _ = writeln!(md, "- {x}");
            }
        }
    }
    md.push_str("\n## Experiments in no entry\n\nRecords of other inputs, of settings across trees, and planned ones. Inconclusive and contradicted verdicts are listed like held ones.\n\n");
    md.push_str("| experiment | kind | tree | purpose | hypothesis | clean passes of ended runs per variant | primary metric per variant, median [min, max] (n) | verdict |\n|---|---|---|---|---|---|---|---|\n");
    for x in &b.others {
        let vs = x.variants.iter().map(|(v, listed, ended, clean, _)| if *ended == 0 { format!("{v} 0 ended of {listed}") } else { format!("{v} {clean} of {ended}") }).collect::<Vec<_>>().join("; ");
        let prim = if x.hypothesis.is_some() {
            format!("`{}`: {}", x.metric.as_deref().unwrap_or(""), x.variants.iter().map(|(v, _, _, _, p)| format!("{v} {}", stat_md(p, super::fmt))).collect::<Vec<_>>().join("; "))
        } else {
            "-".into()
        };
        let _ = writeln!(md, "| {} | {} | {} | {} | {} | {vs} | {prim} | {} |", link(&x.id), x.kind, if x.tree.is_empty() { "-" } else { &x.tree },
            cell_md(&x.purpose), x.hypothesis.as_deref().map_or("-".into(), cell_md), if x.verdict.is_empty() { "-" } else { &x.verdict });
    }
    md
}

fn anchor(e: &Entry) -> String {
    e.label.to_lowercase().chars().filter_map(|c| match c {
        'a'..='z' | '0'..='9' | '-' | '_' => Some(c),
        ' ' => Some('-'),
        _ => None,
    }).collect()
}

// ---- docs/progress.json ------------------------------------------------------------------------

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn stat_j(s: &Option<Stat>) -> J {
    s.as_ref().map_or(J::Null, |s| json!({"n": s.n, "median": r6(s.median), "min": r6(s.min), "max": r6(s.max)}))
}

fn counts_j(c: &Counts) -> J {
    json!({"ended": c.ended, "clean_passes": c.clean, "not_ended": c.open,
        "rate": if c.ended >= SMALL_N { json!(r6(c.clean as f64 / c.ended as f64)) } else { J::Null }, "small_n": c.ended < SMALL_N})
}

fn exp_j(x: &ExpFig) -> J {
    json!({
        "id": x.id, "kind": x.kind, "tree": x.tree, "purpose": x.purpose, "hypothesis": x.hypothesis,
        "primary": x.metric.as_ref().map(|m| json!({"metric": m, "direction": x.direction, "min_effect": x.min_effect})),
        "verdict": x.verdict, "why": x.why,
        "variants": x.variants.iter().map(|(v, listed, ended, clean, p)| json!({"id": v, "listed": listed, "ended": ended, "clean_passes": clean, "primary": stat_j(p)})).collect::<Vec<_>>(),
        "known_confounds": x.confounds.iter().map(|(k, n)| json!({"key": k, "note": n})).collect::<Vec<_>>(),
    })
}

fn render_json(b: &Built) -> String {
    let l = &b.ledger;
    let entries: Vec<J> = l.entries.iter().zip(&b.figs).map(|(e, f)| {
        let cells: Vec<J> = f.cells.iter().map(|x| {
            let rs: Vec<&RunFig> = x.runs.iter().collect();
            let mut v = counts_j(&counts(std::iter::once(x)));
            v["setting"] = json!(l.settings[x.setting].id);
            v["experiment"] = json!(x.experiment);
            v["variant"] = json!(x.variant);
            v["clean_pass_cost_usd"] = stat_j(&clean_stat(&rs, |r| r.cost));
            v["clean_pass_wall_s"] = stat_j(&clean_stat(&rs, |r| r.wall));
            v["runs"] = json!(x.runs.iter().map(|r| json!({"run": r.name, "clean_pass": r.clean, "cost_usd": r.cost.map(r6), "wall_s": r.wall, "end": r.end})).collect::<Vec<_>>());
            v["not_ended_runs"] = json!(x.open);
            v
        }).collect();
        json!({
            "id": e.id, "kind": e.kind, "version": e.version, "label": e.label, "date": e.date, "tree": e.tree, "tree_note": e.tree_note,
            "changes": e.changes.iter().map(|c| json!({"commit": c.commit, "line": c.line, "effect": c.effect, "experiment": c.experiment})).collect::<Vec<_>>(),
            "experiments": f.exps.iter().map(exp_j).collect::<Vec<_>>(),
            "totals": counts_j(&counts(f.cells.iter())),
            "settings": cells,
            "limits": entry_limits(b, e, f),
        })
    }).collect();
    let doc = json!({
        "format": "sdc.progress/1",
        "written_by": "sdc-experiment progress",
        "input": {"name": l.input_name, "sha256": l.input_sha},
        "pass_rule": "clean-delivery: closed, delivered, no reopen, forced step or unstick",
        "rate_from_ended_runs": SMALL_N,
        "settings": l.settings.iter().map(|s| json!({"id": s.id, "label": s.label, "note": s.note, "match": s.matches})).collect::<Vec<_>>(),
        "limits": l.limits,
        "entries": entries,
        "other_experiments": b.others.iter().map(exp_j).collect::<Vec<_>>(),
        "sources": b.sources.iter().map(|(p, h)| json!({"path": p, "sha256": h})).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
}

// ---- docs/img/progress.svg ---------------------------------------------------------------------

/// The image's columns, each a list of entry indices. At most `chart::MAX_COLS`. Until three
/// releases exist every pre-release tree is a column; from then on they are one column. Releases
/// take the remaining columns, the newest first; the older ones are counted as hidden.
fn columns(l: &Ledger) -> (Vec<Vec<usize>>, usize) {
    let pre: Vec<usize> = (0..l.entries.len()).filter(|i| l.entries[*i].kind != "release").collect();
    let rel: Vec<usize> = (0..l.entries.len()).filter(|i| l.entries[*i].kind == "release").collect();
    let fold = rel.len() >= 3 || pre.len() + rel.len().min(1) > chart::MAX_COLS;
    let mut cols: Vec<Vec<usize>> = if fold && !pre.is_empty() { vec![pre] } else { pre.into_iter().map(|i| vec![i]).collect() };
    let room = chart::MAX_COLS - cols.len();
    let hidden = rel.len().saturating_sub(room);
    cols.extend(rel[hidden..].iter().map(|i| vec![*i]));
    (cols, hidden)
}

/// What changed in an entry, in a few words: its `short`.
fn change_note(e: &Entry) -> String {
    e.short.clone()
}

fn chart_model(b: &Built) -> chart::Model {
    let l = &b.ledger;
    let (groups, hidden) = columns(l);
    let cols: Vec<chart::Col> = groups.iter().map(|g| {
        let c = counts(g.iter().flat_map(|i| b.figs[*i].cells.iter()));
        let exps: Vec<String> = g.iter().flat_map(|i| b.figs[*i].exps.iter().map(|x| x.id.clone())).collect();
        let first = &l.entries[g[0]];
        let (label, note) = if g.len() == 1 {
            (first.label.clone(), change_note(first))
        } else {
            let next = l.entries.iter().find(|e| e.kind == "release").map_or("releases".to_string(), |e| e.label.clone());
            ("Trials".to_string(), format!("{} trees before {next}", g.len()))
        };
        chart::Col {
            label,
            note,
            release: first.kind == "release",
            source: match exps.len() {
                0 => "no experiment yet".into(),
                1 | 2 => exps.join(", "),
                k => format!("{k} experiments"),
            },
            measured: !exps.is_empty(),
            ended: c.ended,
            clean: c.clean,
            open: c.open,
        }
    }).collect();
    let rows: Vec<chart::Row> = l.settings.iter().enumerate().filter_map(|(i, s)| {
        let cells: Vec<Option<chart::CellM>> = groups.iter().map(|g| {
            let mine: Vec<&Cell> = g.iter().flat_map(|e| b.figs[*e].cells.iter()).filter(|c| c.setting == i).collect();
            if mine.is_empty() {
                return None;
            }
            let rs: Vec<&RunFig> = mine.iter().flat_map(|c| c.runs.iter()).collect();
            Some(chart::CellM {
                runs: rs.iter().map(|r| r.clean).collect(),
                open: mine.iter().map(|c| c.open.len()).sum(),
                cost: clean_stat(&rs, |r| r.cost).map(|s| s.median),
                wall_s: clean_stat(&rs, |r| r.wall).map(|s| s.median),
                experiments: mine.iter().map(|c| c.experiment.clone()).collect::<BTreeSet<_>>().into_iter().collect(),
            })
        }).collect();
        cells.iter().any(Option::is_some).then(|| chart::Row { label: s.label.clone(), note: s.note.clone(), cells })
    }).collect();
    let dates: BTreeSet<&str> = l.entries.iter().map(|e| e.date.as_str()).collect();
    let range = match (dates.first(), dates.last()) {
        (Some(a), Some(z)) if a != z => format!("{a} to {z}"),
        (Some(a), _) => a.to_string(),
        _ => String::new(),
    };
    let idea = if l.input_description.is_empty() { l.input_name.clone() } else { l.input_description.clone() };
    chart::Model {
        title: "SDC progress: unattended runs from an idea to a delivered app".into(),
        subtitle: format!("{range} · idea: {idea} · clean pass: closed, delivered, no reopen, forced step or unstick"),
        cols,
        rows,
        hidden_releases: hidden,
    }
}

// ---- command and check -------------------------------------------------------------------------

pub(super) struct Rendered {
    pub md: String,
    pub json: String,
    pub svg: String,
    pub problems: Vec<String>,
}

pub(super) fn render(exp: &Path) -> Result<Option<Rendered>, String> {
    Ok(build(exp)?.map(|b| Rendered { md: render_md(&b), json: render_json(&b), svg: chart::render(&chart_model(&b)), problems: b.problems.clone() }))
}

/// The three files, relative to the repository of `exp`.
pub(super) fn outputs(exp: &Path) -> [(PathBuf, &'static str); 3] {
    let docs = repo_of(exp).join("docs");
    [(docs.join("progress.md"), "md"), (docs.join("progress.json"), "json"), (docs.join("img").join("progress.svg"), "svg")]
}

fn pick<'a>(r: &'a Rendered, which: &str) -> &'a str {
    match which {
        "md" => &r.md,
        "json" => &r.json,
        _ => &r.svg,
    }
}

/// Writes the three files when the table is valid. Returns what it did, one line each.
pub(super) fn write(exp: &Path) -> Result<Vec<String>, String> {
    let Some(r) = render(exp)? else { return Ok(vec![]) };
    if !r.problems.is_empty() {
        return Err(r.problems.join("\n"));
    }
    let mut log = vec![];
    for (path, which) in outputs(exp) {
        let text = pick(&r, which);
        if fs::read_to_string(&path).ok().as_deref() != Some(text) {
            if let Some(d) = path.parent() {
                fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
            }
            fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            log.push(format!("wrote {}", path.display()));
        }
    }
    Ok(log)
}

/// The table's problems and every output that is missing or not what the records give.
pub(super) fn check(exp: &Path) -> Result<Vec<String>, String> {
    let Some(r) = render(exp)? else { return Ok(vec![]) };
    let mut p = r.problems.clone();
    for (path, which) in outputs(exp) {
        match fs::read_to_string(&path) {
            Ok(t) if t == pick(&r, which) => {}
            Ok(_) => p.push(format!("{}: stale; run sdc-experiment progress", path.display())),
            Err(_) => p.push(format!("{}: missing; run sdc-experiment progress", path.display())),
        }
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDEA: &str = "A to-do list web app";

    fn summary(run: &str, writer: &str, clean: bool, cost: f64, wall: f64) -> String {
        json!({"format": "sdc.run-metrics/1", "run": run, "nodes": [], "edges": [], "stages": [],
            "metrics": {"finished": true, "closed": true, "delivered": clean, "clean": clean, "cost_usd": cost, "wall_s": wall,
                "end": if clean { "closed: operating" } else { "closed: not_delivered" }},
            "settings": {"models.writer.model": writer, "decider.model": "h"}}).to_string()
    }

    fn record(id: &str, idea: &str, haiku: &[&str], sonnet: &[&str]) -> String {
        let list = |xs: &[&str]| format!("[{}]", xs.join(", "));
        format!("format: sdc.experiment/1\nid: {id}\nkind: retrospective\nstatus: complete\ncreated: 2026-11-21\ntitle: t\npurpose: p\n\
inputs:\n- {{name: todo, kind: idea, text: \"{idea}\", sha256: {}}}\ntree_commit: acd6f12\n\
fixed: {{decider.model: h}}\nvariants:\n- {{id: a, settings: {{models.writer.model: h}}, runs: {}}}\n- {{id: b, settings: {{models.writer.model: s}}, runs: {}}}\n\
runs_per_variant: 2\npass_rule: clean-delivery\ndecision_rule: {{rule: descriptive, text: t}}\n", super::super::sha256_hex(idea), list(haiku), list(sonnet))
    }

    fn table(experiments: &str) -> String {
        format!("format: sdc.releases/1\ninput: {{name: todo, sha256: {}}}\nsettings:\n\
- {{id: haiku, label: All Haiku, note: n, match: {{models.writer.model: h, decider.model: h}}}}\n\
- {{id: mixed, label: Sonnet work, note: n, match: {{models.writer.model: s, decider.model: h}}}}\n\
entries:\n- id: t1\n  kind: pre-release\n  short: a fix\n  date: 2026-11-21\n  tree: acd6f12\n  experiments: {experiments}\n  changes: [{{commit: acd6f12, line: a fix, effect: unmeasured}}]\n\
- id: 0.1.0\n  kind: release\n  short: notes\n  version: 0.1.0\n  date: 2026-11-22\n  tree: null\n  tree_note: not tagged\n  experiments: []\n  changes: [{{commit: 8b3abd4, line: notes, effect: none}}]\n",
            super::super::sha256_hex(IDEA))
    }

    fn fixture(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sdc-progress-unit-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let exp = d.join("experiments");
        fs::create_dir_all(exp.join("e1/runs")).unwrap();
        fs::write(exp.join("e1.yaml"), record("e1", IDEA, &["h1", "h2", "h3"], &["s1", "s2"])).unwrap();
        for (r, w, c, cost, wall) in [("h1", "h", true, 1.0, 100.0), ("h2", "h", false, 0.5, 50.0), ("h3", "h", true, 3.0, 300.0), ("s1", "s", true, 2.0, 60.0)] {
            fs::write(exp.join(format!("e1/runs/{r}.json")), summary(r, w, c, cost, wall)).unwrap();
        }
        fs::write(exp.join(RELEASES), table("[e1]")).unwrap();
        exp
    }

    #[test]
    fn figures_count_clean_passes_of_ended_runs_per_setting() {
        let exp = fixture("figures");
        let b = build(&exp).unwrap().unwrap();
        assert!(b.problems.is_empty(), "{:?}", b.problems);
        let f = &b.figs[0];
        assert_eq!(f.cells.len(), 2);
        let haiku = &f.cells[0];
        assert_eq!((haiku.setting, haiku.runs.len(), haiku.open.len()), (0, 3, 0));
        let rs: Vec<&RunFig> = haiku.runs.iter().collect();
        let cost = clean_stat(&rs, |r| r.cost).unwrap();
        assert_eq!((cost.n, cost.median, cost.min, cost.max), (2, 2.0, 1.0, 3.0));
        let mixed = &f.cells[1];
        assert_eq!((mixed.setting, mixed.runs.len(), mixed.open.clone()), (1, 1, vec!["s2".to_string()]));
        let c = counts(f.cells.iter());
        assert_eq!((c.clean, c.ended, c.open), (3, 4, 1));
        assert_eq!(rate_md(&c), "75 %");
        assert_eq!(rate_md(&counts(std::iter::once(mixed))), "none given, n 1 < 3");
        assert!(b.figs[1].cells.is_empty());
        let md = render_md(&b);
        assert!(md.contains("| All Haiku | `e1` | a | 2 of 3 | 67 % | 2.00 [1.00, 3.00] (2) | 200 [100, 300] (2) | **h1**, h2 (closed: not_delivered), **h3** |"), "{md}");
        assert!(md.contains("Tested by: no experiment yet."), "{md}");
        assert!(md.contains("Listed but not ended, so not counted: `s2`."), "{md}");
        let j: J = serde_json::from_str(&render_json(&b)).unwrap();
        assert_eq!(j["entries"][0]["totals"]["clean_passes"], 3);
        assert_eq!(j["entries"][0]["settings"][1]["rate"], J::Null);
        assert_eq!(j["entries"][0]["settings"][1]["small_n"], true);
        assert_eq!(j["entries"][0]["settings"][0]["clean_pass_wall_s"]["median"], 200.0);
    }

    #[test]
    fn a_run_in_no_setting_a_run_counted_twice_and_another_input_are_problems() {
        let exp = fixture("problems");
        fs::write(exp.join("e2.yaml"), record("e2", "another idea", &["h1"], &[])).unwrap();
        fs::write(exp.join(RELEASES), table("[e1]").replace("tree: null\n  tree_note: not tagged\n  experiments: []", "tree: null\n  tree_note: not tagged\n  experiments: [e2]")
            .replace("match: {models.writer.model: s, decider.model: h}", "match: {models.writer.model: x}")).unwrap();
        let b = build(&exp).unwrap().unwrap();
        let p = b.problems.join("\n");
        assert!(p.contains("variant b: its settings match no setting"), "{p}");
        assert!(p.contains("run h1 is counted in entry \"t1\" as well"), "{p}");
        assert!(p.contains("experiment e2 has no input with the table's sha256"), "{p}");
    }

    #[test]
    fn the_table_fields_are_checked() {
        let text = table("[e1]").replace("version: 0.1.0", "version: 0.1").replace("short: a fix", "short: a change described in far too many words").replace("effect: none", "effect: maybe").replace("tree: acd6f12", "tree: several");
        let y: Y = serde_yaml::from_str(&text).unwrap();
        let (_, p) = parse(&y, &text);
        let p = p.join("\n");
        assert!(p.contains("a release needs `version` (x.y.z)"), "{p}");
        assert!(p.contains("`effect` must be measured, unmeasured or none"), "{p}");
        assert!(p.contains("`tree: several` is for pre-release trees only, with `tree_note`"), "{p}");
        assert!(p.contains("`short` must say what changed in at most 28 characters"), "{p}");
    }

    /// Four pre-release trees and `releases` releases, each measured by its own record. With
    /// `SDC_PROGRESS_RENDER_DIR` set the image is written there as `releases-<n>.svg`, to look at.
    fn many(releases: usize) -> Built {
        let exp = std::env::temp_dir().join(format!("sdc-progress-unit-{}-many-{releases}", std::process::id())).join("experiments");
        let _ = fs::remove_dir_all(&exp);
        let mut entries = String::new();
        for k in 0..4 + releases {
            let id = format!("e{k}");
            fs::create_dir_all(exp.join(&id).join("runs")).unwrap();
            let (h, s): (Vec<String>, Vec<String>) = ((0..3).map(|j| format!("{id}-h{j}")).collect(), (0..3).map(|j| format!("{id}-s{j}")).collect());
            let hr: Vec<&str> = h.iter().map(String::as_str).collect();
            let sr: Vec<&str> = s.iter().map(String::as_str).collect();
            fs::write(exp.join(format!("{id}.yaml")), record(&id, IDEA, &hr, &sr)).unwrap();
            for (j, r) in h.iter().enumerate() {
                fs::write(exp.join(&id).join("runs").join(format!("{r}.json")), summary(r, "h", j < k.min(3), 1.0 + k as f64 * 0.1, 1800.0)).unwrap();
            }
            for r in &s {
                fs::write(exp.join(&id).join("runs").join(format!("{r}.json")), summary(r, "s", true, 2.5, 600.0 + k as f64 * 30.0)).unwrap();
            }
            let (kind, version) = if k < 4 { ("pre-release", String::new()) } else { ("release", format!("\n  version: 0.{}.0", k - 3)) };
            let eid = if k < 4 { format!("t{k}") } else { format!("0.{}.0", k - 3) };
            let _ = write!(entries, "- id: {eid}\n  kind: {kind}{version}\n  short: change {k}\n  date: 2026-12-{:02}\n  tree: acd6f12\n  experiments: [{id}]\n  changes: [{{commit: acd6f12, line: change number {k} with a long line to cut, effect: unmeasured}}]\n", 1 + k);
        }
        let t = table("[]");
        let head = &t[..t.find("entries:\n").unwrap() + "entries:\n".len()];
        fs::write(exp.join(RELEASES), format!("{head}{entries}")).unwrap();
        let b = build(&exp).unwrap().unwrap();
        assert!(b.problems.is_empty(), "{:?}", b.problems);
        if let Some(d) = std::env::var_os("SDC_PROGRESS_RENDER_DIR") {
            fs::write(Path::new(&d).join(format!("releases-{releases}.svg")), chart::render(&chart_model(&b))).unwrap();
        }
        b
    }

    #[test]
    fn the_image_folds_pre_release_trees_from_three_releases_and_keeps_the_newest_releases() {
        let one = chart_model(&many(1));
        assert_eq!((one.cols.len(), one.hidden_releases), (5, 0));
        assert_eq!(one.cols[4].label, "0.1.0");
        let five = chart_model(&many(5));
        assert_eq!((five.cols.len(), five.hidden_releases), (6, 0));
        assert_eq!((five.cols[0].label.as_str(), five.cols[0].note.as_str()), ("Trials", "4 trees before 0.1.0"));
        assert_eq!(five.cols[0].ended, 24);
        let fifteen = chart_model(&many(15));
        assert_eq!((fifteen.cols.len(), fifteen.hidden_releases), (6, 10));
        assert_eq!(fifteen.cols[1].label, "0.11.0");
        assert_eq!(fifteen.cols[5].label, "0.15.0");
    }

    #[test]
    fn the_chart_is_the_same_bytes_for_the_same_inputs() {
        let a = render(&fixture("svg-a")).unwrap().unwrap();
        let b = render(&fixture("svg-b")).unwrap().unwrap();
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.json.replace("svg-a", ""), b.json.replace("svg-b", ""));
        assert!(!has_abs_path(&a.svg));
        assert!(a.svg.contains("font-family:system-ui,sans-serif"));
        assert!(a.svg.contains(">2 of 3<"), "{}", a.svg);
    }
}
