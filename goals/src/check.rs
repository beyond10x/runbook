//! Validation, digests and resolution of `sdc.goals/1` documents. Rules and fields:
//! docs/requester.md § 2.

use serde_json::Value as Json;
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const FORMAT: &str = "sdc.goals/1";
/// Highest layer first; a lower layer overrides a higher one.
pub const LAYERS: [&str; 3] = ["company", "product", "request"];
pub const KINDS: [&str; 4] = ["goal", "principle", "constraint", "preference"];
const FILE_FIELDS: [&str; 4] = ["format", "layer", "requester", "entries"];
const ENTRY_FIELDS: [&str; 11] = [
    "id",
    "statement",
    "kind",
    "priority",
    "sources",
    "stated",
    "derived_from",
    "valid_from",
    "valid_until",
    "overrides",
    "rule",
];

#[derive(Debug, Clone)]
pub struct Finding {
    pub file: String,
    /// Entry id, or "-" for a finding about the file or an entry without a usable id.
    pub id: String,
    pub code: &'static str,
    pub message: String,
    order: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub file: String,
    pub id: String,
    pub layer: &'static str,
    pub kind: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub overrides: Option<String>,
    pub derived_from: Vec<String>,
    /// The entry's fields as parsed, for the digest and for resolve.
    pub raw: Json,
}

impl Entry {
    pub fn digest(&self) -> String {
        digest_of(&self.raw)
    }
}

pub struct Report {
    pub entries: Vec<Entry>,
    pub findings: Vec<Finding>,
}

struct Doc {
    name: String,
    text: String,
}

pub fn check(files: &[(String, String)]) -> Report {
    let docs: Vec<Doc> = files
        .iter()
        .map(|(n, t)| Doc {
            name: n.clone(),
            text: t.clone(),
        })
        .collect();
    let mut findings: Vec<Finding> = Vec::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();

    for (fi, d) in docs.iter().enumerate() {
        let mut f = |entry: usize, id: &str, code: &'static str, message: String| {
            findings.push(Finding {
                file: d.name.clone(),
                id: id.to_string(),
                code,
                message,
                order: (fi, entry),
            });
        };
        let root: Value = match serde_yaml::from_str(&d.text) {
            Ok(v) => v,
            Err(e) => {
                f(0, "-", "parse", format!("not valid YAML: {e}"));
                continue;
            }
        };
        let Some(root) = root.as_mapping() else {
            f(0, "-", "parse", "the document is not a mapping".into());
            continue;
        };
        for k in root.keys() {
            let name = k.as_str().unwrap_or("?");
            if !FILE_FIELDS.contains(&name) {
                f(
                    0,
                    "-",
                    "unknown_field",
                    format!("file-level key `{name}` is not part of {FORMAT}"),
                );
            }
        }
        match root.get("format").and_then(Value::as_str) {
            Some(FORMAT) => {}
            Some(other) => f(
                0,
                "-",
                "format",
                format!("format is `{other}`, expected `{FORMAT}`"),
            ),
            None => f(
                0,
                "-",
                "format",
                format!("format marker `{FORMAT}` is missing"),
            ),
        }
        if root.get("requester").is_some_and(|r| !r.is_string()) {
            f(
                0,
                "-",
                "unknown_field",
                "`requester` must be a string".into(),
            );
        }
        let file_layer: Option<&'static str> = match root.get("layer").and_then(Value::as_str) {
            Some(l) => match LAYERS.iter().find(|x| **x == l) {
                Some(x) => Some(x),
                None => {
                    f(
                        0,
                        "-",
                        "layer",
                        format!("layer `{l}` is not one of company, product, request"),
                    );
                    None
                }
            },
            None => {
                f(
                    0,
                    "-",
                    "layer",
                    "layer is missing; it is one of company, product, request".into(),
                );
                None
            }
        };
        let Some(list) = root.get("entries").and_then(Value::as_sequence) else {
            f(
                0,
                "-",
                "missing_entries",
                "`entries` is missing or not a list".into(),
            );
            continue;
        };
        for (i, item) in list.iter().enumerate() {
            let n = i + 1;
            let Some(m) = item.as_mapping() else {
                f(n, "-", "bad_entry", format!("entry {n} is not a mapping"));
                continue;
            };
            for k in m.keys() {
                let name = k.as_str().unwrap_or("?");
                if !ENTRY_FIELDS.contains(&name) && name != "source" {
                    f(
                        n,
                        str_of(m, "id").as_deref().unwrap_or("-"),
                        "unknown_field",
                        format!("field `{name}` is not part of {FORMAT}"),
                    );
                }
            }
            let id = str_of(m, "id").filter(|s| !s.trim().is_empty());
            let label = id.clone().unwrap_or_else(|| "-".to_string());
            let mut layer: Option<&'static str> = None;
            match &id {
                None => f(n, "-", "missing_id", format!("entry {n} has no id")),
                Some(id) => {
                    match split_id(id) {
                        None => f(n, id, "bad_id", "id must be `<layer>/<name>`, e.g. company/c2".into()),
                        Some((prefix, _)) => match LAYERS.iter().find(|l| **l == prefix) {
                            None => f(n, id, "unknown_layer", format!("layer prefix `{prefix}` is not one of company, product, request")),
                            Some(l) => {
                                layer = Some(l);
                                if file_layer.is_some_and(|fl| fl != *l) {
                                    f(n, id, "layer_mismatch", format!("prefix `{l}` differs from the file's layer `{}`", file_layer.unwrap()));
                                }
                            }
                        },
                    }
                    if let Some(first) = seen.get(id) {
                        f(
                            n,
                            id,
                            "duplicate_id",
                            format!("id already defined in {first}"),
                        );
                    } else {
                        seen.insert(id.clone(), d.name.clone());
                    }
                }
            }

            let kind = str_of(m, "kind");
            match kind.as_deref() {
                Some(k) if KINDS.contains(&k) => {}
                Some(k) => f(
                    n,
                    &label,
                    "unknown_kind",
                    format!("kind `{k}` is not goal, principle, constraint or preference"),
                ),
                None => f(n, &label, "unknown_kind", "kind is missing".into()),
            }
            if str_of(m, "statement").is_none_or(|s| s.trim().is_empty()) {
                f(
                    n,
                    &label,
                    "missing_statement",
                    "statement is missing or empty".into(),
                );
            }

            let mut sources = Vec::new();
            if m.contains_key("source") {
                f(n, &label, "source_form", "use sources: [...]".into());
            } else {
                match m.get("sources").and_then(Value::as_sequence) {
                    Some(l) if !l.is_empty() => {
                        for s in l {
                            match s.as_str() {
                                Some(s) if source_form(s) => sources.push(s.to_string()),
                                other => f(n, &label, "source_form", format!(
                                    "source `{}` is not exactly one of person:<id>, aep:<id>@<rev>, <path>:<line>, decision:<request id>",
                                    other.map_or_else(|| format!("{s:?}"), str::to_string))),
                            }
                        }
                    }
                    _ => f(
                        n,
                        &label,
                        "missing_source",
                        "sources is missing, empty or not a list".into(),
                    ),
                }
            }
            if let Some(s) = m.get("stated") {
                let ok = s.as_mapping().is_some_and(|s| {
                    s.len() == 2
                        && str_of(s, "by").is_some_and(|b| !b.trim().is_empty())
                        && str_of(s, "at").is_some_and(|a| instant(&a).is_some())
                });
                if !ok {
                    f(
                        n,
                        &label,
                        "bad_stated",
                        "stated is {by: <who>, at: <date>} and nothing else".into(),
                    );
                }
            }
            let mut derived_from = Vec::new();
            if let Some(d) = m.get("derived_from") {
                let list: Option<Vec<&str>> = d
                    .as_sequence()
                    .filter(|l| !l.is_empty())
                    .and_then(|l| l.iter().map(Value::as_str).collect());
                match list {
                    Some(l) if l.iter().all(|x| split_id(x).is_some()) => {
                        derived_from = l.iter().map(|x| x.to_string()).collect()
                    }
                    _ => f(
                        n,
                        &label,
                        "bad_derived_from",
                        "derived_from is a non-empty list of entry ids".into(),
                    ),
                }
            }

            match (m.get("priority"), kind.as_deref()) {
                (Some(_), Some("constraint")) => f(
                    n,
                    &label,
                    "priority_on_constraint",
                    "a constraint is hard and has no priority".into(),
                ),
                (None, Some("constraint")) => {}
                (None, _) => f(
                    n,
                    &label,
                    "missing_priority",
                    "priority is missing (1 to 5, 1 is highest)".into(),
                ),
                (Some(p), _) => match p.as_u64().filter(|p| (1..=5).contains(p)) {
                    Some(_) => {}
                    None => f(
                        n,
                        &label,
                        "bad_priority",
                        "priority must be an integer from 1 to 5 (1 is highest)".into(),
                    ),
                },
            }

            let valid_from = date_field(m, "valid_from", true, n, &label, &mut f);
            let valid_until = date_field(m, "valid_until", false, n, &label, &mut f);
            if let (Some(a), Some(b)) = (&valid_from, &valid_until) {
                if b <= a {
                    f(
                        n,
                        &label,
                        "window_order",
                        format!("valid_until {b} is not after valid_from {a}"),
                    );
                }
            }

            if m.contains_key("rule") && kind.as_deref() != Some("constraint") {
                f(
                    n,
                    &label,
                    "rule_not_constraint",
                    "only a constraint carries a rule".into(),
                );
            }
            let overrides = match m.get("overrides") {
                None => None,
                Some(Value::String(s)) if split_id(s).is_some_and(|(p, _)| LAYERS.contains(&p)) => {
                    Some(s.clone())
                }
                Some(_) => {
                    f(
                        n,
                        &label,
                        "bad_overrides",
                        "overrides must name an entry id `<layer>/<name>`".into(),
                    );
                    None
                }
            };

            if let (Some(id), Some(layer)) = (id, layer) {
                if !entries.iter().any(|e| e.id == id) {
                    let raw = serde_json::to_value(item).unwrap_or(Json::Null);
                    entries.push(Entry {
                        file: d.name.clone(),
                        id,
                        layer,
                        kind,
                        valid_from,
                        valid_until,
                        overrides,
                        derived_from,
                        raw,
                    });
                }
            }
        }
    }

    cross_layer(&docs, &entries, &mut findings);
    findings.sort_by_key(|f| f.order);
    Report { entries, findings }
}

/// Overrides and derivations across the files given together.
fn cross_layer(docs: &[Doc], entries: &[Entry], out: &mut Vec<Finding>) {
    let by_id: BTreeMap<&str, &Entry> = entries.iter().map(|e| (e.id.as_str(), e)).collect();
    for (pos, e) in entries.iter().enumerate() {
        let fi = docs.iter().position(|d| d.name == e.file).unwrap_or(0);
        let mut f = |code: &'static str, message: String| {
            out.push(Finding {
                file: e.file.clone(),
                id: e.id.clone(),
                code,
                message,
                order: (fi, pos + 1),
            });
        };
        let rank = |l: &str| LAYERS.iter().position(|x| *x == l).unwrap();
        if let Some(target) = &e.overrides {
            let (tp, _) = split_id(target).unwrap();
            if rank(tp) >= rank(e.layer) {
                f(
                    "override_layer",
                    format!(
                        "overrides {target}, which is not in a higher layer than {}",
                        e.layer
                    ),
                );
            } else {
                match by_id.get(target.as_str()) {
                    None => f("override_target_missing", format!("overrides {target}, which no given file defines; pass every higher layer")),
                    Some(t) => match t.kind.as_deref() {
                        Some("constraint") => f("override_constraint", format!("overrides {target}, a constraint; a constraint is never overridden")),
                        Some("goal") => f("override_kind", format!("overrides {target}, a goal; only a principle or a preference is overridden")),
                        _ => {}
                    },
                }
            }
        }
        for t in &e.derived_from {
            if !by_id.contains_key(t.as_str()) {
                f(
                    "derived_unknown",
                    format!("derived_from {t}, which no given file defines"),
                );
            }
        }
    }
}

/// Entries that apply at an instant, `at` normalised by `instant`: inside their window
/// (`valid_from` <= at < `valid_until`), minus the entries an applying override replaces.
/// Ordered by layer (company, product, request), then as given.
pub fn resolve<'a>(entries: &'a [Entry], at: &str) -> Vec<&'a Entry> {
    let live: Vec<&Entry> = entries
        .iter()
        .filter(|e| {
            e.valid_from.as_deref().is_some_and(|v| v <= at)
                && e.valid_until.as_deref().is_none_or(|u| at < u)
        })
        .collect();
    let replaced: Vec<&str> = live.iter().filter_map(|e| e.overrides.as_deref()).collect();
    let mut out: Vec<&Entry> = live
        .into_iter()
        .filter(|e| !replaced.contains(&e.id.as_str()))
        .collect();
    out.sort_by_key(|e| LAYERS.iter().position(|l| *l == e.layer));
    out
}

/// Why a goals ground does not hold: a code and a message naming the cause.
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

/// A cited goals entry, `goals@<revision>#<id>`, checked against the given layers at an instant.
pub struct Grounded<'a> {
    pub entry: Option<&'a Entry>,
    pub refusals: Vec<Refusal>,
}

/// Parses `goals@<revision>#<entry id>` into (revision, id).
pub fn parse_cite(cite: &str) -> Option<(&str, &str)> {
    let rest = cite.strip_prefix("goals@")?;
    let (rev, id) = rest.split_once('#')?;
    (!rev.is_empty() && split_id(id).is_some()).then_some((rev, id))
}

/// Checks one goals ground (docs/requester.md § 2, decision 3): the cited revision is the current
/// one, the entry exists in the given layers, its digest is the cited one (when one is cited), it
/// is inside its validity window at `at`, and no entry of a lower layer that applies at `at`
/// overrides it. Every failing check is one refusal; `report` must have no findings.
pub fn ground<'a>(
    report: &'a Report,
    revision: &str,
    current: &str,
    id: &str,
    digest: Option<&str>,
    at: &str,
) -> Grounded<'a> {
    let mut refusals = Vec::new();
    let mut no = |code, message| refusals.push(Refusal { code, message });
    if revision != current {
        no(
            "not_current",
            format!("goals@{revision} is not the current revision ({current})"),
        );
    }
    let entry = report.entries.iter().find(|e| e.id == id);
    match entry {
        None => no(
            "no_entry",
            format!("no entry {id} in the given goals layers"),
        ),
        Some(e) => {
            let now = e.digest();
            if let Some(d) = digest.filter(|d| *d != now) {
                no(
                    "digest_changed",
                    format!("{id} changed: its digest is {now}, the ground cites {d}"),
                );
            }
            match (&e.valid_from, &e.valid_until) {
                (Some(f), _) if at < f.as_str() => no(
                    "not_yet_valid",
                    format!("{id} applies from {f}, after {at}"),
                ),
                (_, Some(u)) if at >= u.as_str() => {
                    no("expired", format!("{id} applied until {u}, not at {at}"))
                }
                _ => {}
            }
            let live = resolve(&report.entries, at);
            if let Some(o) = live.iter().find(|o| o.overrides.as_deref() == Some(id)) {
                no(
                    "overridden",
                    format!("{id} is overridden by {} ({}) at {at}", o.id, o.layer),
                );
            }
        }
    }
    Grounded { entry, refusals }
}

/// The entry with this id as parsed, without any validation.
pub fn raw_entry(text: &str, id: &str) -> Option<Json> {
    let root: Value = serde_yaml::from_str(text).ok()?;
    let found = root
        .get("entries")?
        .as_sequence()?
        .iter()
        .find(|e| e.get("id").and_then(Value::as_str) == Some(id))?;
    serde_json::to_value(found).ok()
}

/// `sha256:<hex>` over the canonical JSON of an entry.
pub fn digest_of(raw: &Json) -> String {
    let mut s = String::new();
    canonical(raw, &mut s);
    let h = Sha256::digest(s.as_bytes());
    format!(
        "sha256:{}",
        h.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

/// Keys sorted, no insignificant whitespace, strings as written.
fn canonical(v: &Json, out: &mut String) {
    match v {
        Json::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                canonical(&m[k], out);
            }
            out.push('}');
        }
        Json::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}

fn str_of(m: &Mapping, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(str::to_string)
}

fn split_id(id: &str) -> Option<(&str, &str)> {
    let (p, n) = id.split_once('/')?;
    (!p.is_empty() && !n.trim().is_empty() && !id.contains(char::is_whitespace)).then_some((p, n))
}

/// Exactly one of `person:<id>`, `aep:<id>@<rev>`, `<path>:<line>`, `decision:<request id>`; no spaces.
fn source_form(s: &str) -> bool {
    if s.contains(char::is_whitespace) {
        return false;
    }
    if let Some(r) = s
        .strip_prefix("person:")
        .or_else(|| s.strip_prefix("decision:"))
    {
        return !r.is_empty();
    }
    if let Some(r) = s.strip_prefix("aep:") {
        return r
            .rsplit_once('@')
            .is_some_and(|(id, rev)| !id.is_empty() && !rev.is_empty());
    }
    s.rsplit_once(':').is_some_and(|(path, line)| {
        !path.is_empty()
            && !line.is_empty()
            && line.bytes().all(|b| b.is_ascii_digit())
            && line.parse::<u64>().is_ok_and(|l| l >= 1)
    })
}

/// A date field as a normalised instant, reporting a finding when missing (if required) or malformed.
fn date_field(
    m: &Mapping,
    key: &str,
    required: bool,
    n: usize,
    label: &str,
    f: &mut impl FnMut(usize, &str, &'static str, String),
) -> Option<String> {
    match m.get(key) {
        None if required => {
            f(
                n,
                label,
                "missing_valid_from",
                "valid_from is missing".into(),
            );
            None
        }
        None => None,
        Some(v) => match v.as_str().and_then(instant) {
            Some(i) => Some(i),
            None => {
                f(
                    n,
                    label,
                    "bad_date",
                    format!("{key} must be YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ, a real date"),
                );
                None
            }
        },
    }
}

/// `YYYY-MM-DD` (midnight UTC) or `YYYY-MM-DDTHH:MM:SSZ`, as the latter, so strings compare in time order.
pub fn instant(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let t = s.get(r)?;
        t.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| t.parse().ok())?
    };
    let full = match b.len() {
        10 => false,
        20 => true,
        _ => return None,
    };
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if d < 1 || d > days {
        return None;
    }
    let (h, mi, se) = if full {
        if b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
            return None;
        }
        (num(11..13)?, num(14..16)?, num(17..19)?)
    } else {
        (0, 0, 0)
    };
    if h > 23 || mi > 59 || se > 59 {
        return None;
    }
    Some(format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}Z"))
}
