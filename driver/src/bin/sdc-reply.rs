//! sdc-reply: what an LLM reply may cite, and the JSON schema of the reply, built by program.
//!
//!   sdc-reply citable --cwd <call dir> --case-dir <case>
//!   sdc-reply resolve --cwd <call dir> --case-dir <case> <cite>...
//!   sdc-reply schema  --cwd <call dir> --case-dir <case> --type approval|question|finding|close
//!                     [--final] [--options '<JSON list>'] [--rule]
//!   sdc-reply rejections --case-dir <case> --action <approval action>
//!   sdc-reply later --ir <ir.json> --action <action>
//!   sdc-reply review-inputs --cwd <call dir> --case-dir <case> --story <story>
//!   sdc-reply schema --cwd <call dir> --case-dir <case> --type review
//!   sdc-reply check-review --cwd <call dir> --case-dir <case> --reply <file> --principal <p>
//!   sdc-reply adapters --tools <tools dir>
//!   sdc-reply implement-run --rec <call record> --cwd <call dir> --case-dir <case> --story <story>
//!                     --adapter <adapter program> [--rework-limit N] [--assemble-only]
//!   sdc-reply implement-pack --tree <dir> [--note <text>]
//!   sdc-reply schema --cwd <call dir> --case-dir <case> --type implement
//!   sdc-reply rule-approval --request <approval request file> --ends <rule-ends.json>
//!   sdc-reply coverage [--work <work>] [--case-dir <case>] [--story <story>] [--schema]
//!   sdc-reply check-coverage [--work <work>] [--case-dir <case>] [--story <story>] --tree <tree> [--reply <file>] [--shape]
//!   sdc-reply coverage-stub [--work <work>] [--case-dir <case>] [--story <story>] --tree <tree>
//!
//! change.implement as one request (`SDC_IMPLEMENT_MODE=request`, reply/implement (src/reply/implement/),
//! docs/driver.md "Implement as one request"): `implement-run` assembles the node's whole input
//! into one prompt, calls the adapter, checks the reply against the schema and the path rule,
//! retries an invalid reply at most twice with the check's error, continues by rule for paths a
//! valid reply left `remaining`, and writes the files into `<cwd>/out/impl/`. `adapters` adds the
//! class `implement` (node change.implement) in request mode: SDC_IMPLEMENT_ADAPTER, else as the
//! review's; an SDC_IMPLEMENT_MODE other than session or request is refused.
//!
//! `rule-approval` (docs/driver.md, "Approvals by program"): whether `rule:autopilot` takes an
//! approval because the program checks guarding it (`approvals.<action>.by_rule` in rule-ends.json)
//! recorded a passing result for the exact revision being approved. It reads the claims from the
//! request and re-reads the evidence on disk. Prints `{take: false, covered, why}` (exit 0) or
//! `{take: true, rule, decision}`: the decision line for decisions.jsonl, citing the evidence ids.
//! An action with no rule prints `covered: false`; exit 64 only when an input is unreadable.
//!
//! change.implement (story:implement-cites-requirements): besides its change, the implementer
//! replies with a coverage document, per requirement the story carries (review_scope's story
//! requirements) `built` with the file and line that builds it and a test, or `not_built` with a
//! reason. `coverage --schema` prints its reply schema; `check-coverage --shape` checks a reply
//! against it (`sdc-node` asks again with the reason, at most twice); `sdc-node` puts a valid one
//! at `impl/.sdc/coverage.json`. Without `--shape`, `check-coverage` is the implement binding's
//! `postcondition`, run after the writer (by `sdc-node` under Loom): exit 1 refuses the revision, naming
//! the requirement ids (missing, named twice, not built, a file not in the tree, a line outside
//! its file, a test the test sources lack); nothing is recorded and the writer is sent back.
//! `coverage-stub` writes the stub implementer's document. review-inputs copies the document to
//! the review's `in/coverage.json`.
//!
//! `adapters`: the adapter of every node class whose reply is a data document made under a
//! response schema (`schema_reply` in tools/node-classes.json: review), and whether it can take a
//! schema. The class's adapter: SDC_<CLASS>_ADAPTER, else claude-cli when SDC_LLM_ADAPTER is
//! metaharness (it has no response-schema option), else SDC_LLM_ADAPTER (stub when unset).
//! claude-cli takes a schema, metaharness none, stub and dry call no model; an adapter program
//! (a path) answers `<path> --capabilities` with `{"json_schema": true|false}`. Prints
//! `{<class>: {adapter, source, json_schema, nodes}}`; exit 64 (stderr names the class and
//! the adapter) when a class's adapter cannot take a schema.
//!
//! change.review (`sdc-node`): `review-inputs` writes the review's inputs into `in/`. With
//! SDC_REVIEW_MODE unset or `session` (the default) they are the story, its acceptance lines, the
//! requirements it delivers and those no story covers, the covered commands, the whole change tree,
//! its diff against the mainline and the gate and test logs. With `facts` (story:review-minimal-facts)
//! they are the one document `review.md` the model reads (the requirements the story delivers and
//! those no story covers, its acceptance lines, the covered commands, the test summary of the
//! conformance run, the diff against the mainline, the full text of only the files the diff touches,
//! the build constraints when the tree has them, a listing of the other product files) and, for the
//! checks, `scope.json`, `uncovered.json`, `change.diff`, `listing.json` and the touched files under
//! `impl/`; it prints `document_bytes`;
//! `schema --type review` prints the reply schema; `check-review` checks the reply: exit 0
//! approved, 1 rejected (stdout: the sdc.rejection/1 that sends the implementer back), 2 invalid
//! (stderr: why; nothing is recorded).
//!
//! Where the facts come from (`Facts`): the old driver's case directory (`case.json`, the event
//! log, the evidence and the blobs) or, with `LOOM_CASE` set, record files of the workspace
//! (`.sdc/records/` of the directory the program was started in, the workspace, whatever `--case-dir`
//! names; Loom gives a bound program its working directory and nothing
//! else). The revision of an artifact is then the digest of its tree (`impl`, `spec`); the log of
//! the conformance run and of the gate is `<kind>/<digest>.log`, written by `sdc-reply record
//! --kind <kind> --tree <tree> --log <file>` from the tools of tests.run and spec.gate; the
//! review's own rejections are `rejections/change.review/<n>.json`, written by `check-review`.
//! `citable`, `resolve`, `schema`, `rejections`, `review-inputs` and `check-review` read them
//! through the same functions in both modes; no `<artifact>@<revision>` cite resolves under Loom.
//!
//! The call dir holds the inputs the model was shown (`in/`), as the decider of `runbook run` writes them.
//! Citable (the forms of sdc.decision.Citation, ess/domains/decision.yaml, docs/decisions.md):
//! - an id: a requirement, dissection item, question or finding id of the case's `work/`, a
//!   context field (`context.<field>`), or a claim id of `in/checks.json`;
//! - `<artifact>@<revision>[#<entry>]` (artifact_revision): an artifact of the case's bindings
//!   whose file is in `in/`, at its current revision in `case.json`; the entry is a dotted path
//!   into a YAML or JSON file, a list element named by its `id` (`requirements.r1`);
//! - `<path>:<line>[-<line_end>]` (path_line): a file under `in/` (written without `in/`), the
//!   lines inside it; it is recorded as `work/<path>`, which resolves against the case directory.
//!
//! `citable` prints the enumerable set (ids, every artifact entry two levels deep, every non-empty
//! line of a Markdown input), the artifacts, and one example of each form from the actual input.
//! `resolve` prints one line per cite with its form and value (exit 0 when all resolve, else 1).
//! `schema` prints the reply schema of the decider of `runbook run`: `decision` an enum of the admissible
//! decisions, `cites` an enum of the citable set, `answer` the options of a rule finding.
//! `reply_schema` and `citable_set` are the parts another LLM node's schema can reuse.
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sdc_tools::reply::adapters::adapters;
use sdc_tools::reply::citations::{citable_set, ctx, resolve};
use sdc_tools::reply::coverage::{coverage_stub, run_check_coverage, story_arg, story_requirements, work_arg};
use sdc_tools::reply::implement;
use sdc_tools::reply::records::Facts;
use sdc_tools::reply::review::{check_review, review_inputs, review_inputs_session, review_mode};
use sdc_tools::reply::rules::{later, rule_approval};
use sdc_tools::reply::schemas::{coverage_schema, decider_schema, review_schema};
use sdc_tools::reply::read_json;
use serde_json::{json, Value};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Citable { #[arg(long)] cwd: PathBuf, #[arg(long)] case_dir: PathBuf },
    Resolve { #[arg(long)] cwd: PathBuf, #[arg(long)] case_dir: PathBuf, cites: Vec<String> },
    Schema {
        #[arg(long)] cwd: PathBuf,
        #[arg(long)] case_dir: PathBuf,
        #[arg(long = "type")] kind: String,
        #[arg(long)] r#final: bool,
        #[arg(long, default_value = "[]")] options: String,
        #[arg(long)] rule: bool,
    },
    /// The rejected records of an approval action in a case, oldest first, each with the reasons
    /// its step kept (the sdc.rejection/1 log blob of `approve --result rejected`).
    Rejections { #[arg(long)] case_dir: PathBuf, #[arg(long)] action: String },
    /// Under Loom: keep the log of a step (a file) as the record of `kind` about the tree `tree`
    /// of the workspace, where `review-inputs` reads it (the tools of tests.run and spec.gate).
    Record {
        #[arg(long, default_value = ".")] work: PathBuf,
        #[arg(long)] kind: String,
        #[arg(long)] tree: String,
        #[arg(long)] log: PathBuf,
    },
    /// What can only come after `action` (from the compiled protocol): the actions whose
    /// precondition needs a claim only it or they make true, the claims only those actions make
    /// true, and the obligations only those claims discharge. A decision about `action` must not
    /// show them as open conditions (runs/trial-3-mixed-3: health.required before release.publish).
    Later { #[arg(long)] ir: PathBuf, #[arg(long)] action: String },
    /// Write change.review's inputs into `<cwd>/in/`; print what was written.
    ReviewInputs { #[arg(long)] cwd: PathBuf, #[arg(long)] case_dir: PathBuf, #[arg(long)] story: String },
    /// Check a change.review reply: exit 0 approved, 1 rejected (the rejection on stdout), 2 invalid.
    CheckReview {
        #[arg(long)] cwd: PathBuf,
        #[arg(long)] case_dir: PathBuf,
        #[arg(long)] reply: PathBuf,
        #[arg(long)] principal: String,
    },
    /// The adapter of every schema-reply node class and whether it takes a schema (exit 64 if not).
    Adapters { #[arg(long)] tools: PathBuf },
    /// change.implement as one request (SDC_IMPLEMENT_MODE=request): assemble the input, ask,
    /// check the reply, retry or continue by rule, write the files into `<cwd>/out/impl/`.
    ImplementRun {
        #[arg(long)] rec: PathBuf,
        #[arg(long)] cwd: PathBuf,
        #[arg(long)] case_dir: PathBuf,
        #[arg(long)] story: String,
        #[arg(long)] adapter: Option<PathBuf>,
        #[arg(long)] rework_limit: Option<u64>,
        #[arg(long)] assemble_only: bool,
        #[arg(long, default_value_t = implement::INLINE_CAP)] max_bytes: usize,
    },
    /// A change.implement reply holding every product file of a tree (the fixture adapter).
    ImplementPack { #[arg(long)] tree: PathBuf, #[arg(long, default_value = "")] note: String },
    /// Whether an approval request is taken by `rule:autopilot` (its program checks passed on the
    /// exact revision) instead of the decider model; prints `{take, why | decision, rule}`.
    RuleApproval { #[arg(long)] request: PathBuf, #[arg(long)] ends: PathBuf },
    /// change.implement's coverage document: `--shape` checks a reply against the schema (exit 0
    /// fits, 2 not: stderr says why); without it, the document in the tree (or `--reply`) against
    /// the change tree: exit 0 complete, 1 refused (stdout names the requirement ids), 2 when it
    /// cannot check. `--case-dir` and `--story` default to the driver's SDC_CASE_DIR and SDC_CASE.
    CheckCoverage {
        #[arg(long)] work: Option<PathBuf>,
        #[arg(long)] case_dir: Option<PathBuf>,
        #[arg(long)] story: Option<String>,
        #[arg(long)] tree: PathBuf,
        #[arg(long)] reply: Option<PathBuf>,
        #[arg(long)] shape: bool,
    },
    /// The requirements the story carries, `[{id, text}]`, and with `--schema` the coverage schema.
    Coverage {
        #[arg(long)] work: Option<PathBuf>,
        #[arg(long)] case_dir: Option<PathBuf>,
        #[arg(long)] story: Option<String>,
        #[arg(long)] schema: bool,
    },
    /// The stub implementer's coverage document for a tree (no model).
    CoverageStub {
        #[arg(long)] work: Option<PathBuf>,
        #[arg(long)] case_dir: Option<PathBuf>,
        #[arg(long)] story: Option<String>,
        #[arg(long)] tree: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Citable { cwd, case_dir } => {
            println!("{}", citable_set(&ctx(&cwd, &Facts::open(&case_dir))));
            ExitCode::SUCCESS
        }
        Cmd::Resolve { cwd, case_dir, cites } => {
            let c = ctx(&cwd, &Facts::open(&case_dir));
            let rs: Vec<Value> = cites.iter().map(|x| resolve(&c, x)).collect();
            let ok = rs.iter().all(|r| r["ok"] == true);
            println!("{}", Value::Array(rs));
            if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Cmd::Schema { cwd, case_dir, kind, r#final, options, rule } => {
            if kind == "review" {
                println!("{}", review_schema(&cwd.join("in")));
                return ExitCode::SUCCESS;
            }
            if kind == "implement" {
                println!("{}", implement::schema());
                return ExitCode::SUCCESS;
            }
            let opts: Vec<String> = serde_json::from_str(&options).unwrap_or_default();
            println!("{}", decider_schema(&ctx(&cwd, &Facts::open(&case_dir)), &kind, r#final, &opts, rule));
            ExitCode::SUCCESS
        }
        Cmd::Rejections { case_dir, action } => {
            println!("{}", Facts::open(&case_dir).rejections(&action));
            ExitCode::SUCCESS
        }
        Cmd::Record { work, kind, tree, log } => match fs::read_to_string(&log).map_err(|e| format!("{}: {e}", log.display()))
            .and_then(|text| Facts::workspace(&work).write_record(&kind, &tree, &text)) {
            Ok(p) => {
                println!("{}", p.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-reply record: {e}");
                ExitCode::from(2)
            }
        },
        Cmd::Later { ir, action } => {
            let ir = read_json(&ir);
            if ir["actions"].get(&action).is_none() {
                eprintln!("sdc-reply later: no action `{action}` in the protocol");
                return ExitCode::from(64);
            }
            println!("{}", later(&ir, &action));
            ExitCode::SUCCESS
        }
        Cmd::ReviewInputs { cwd, case_dir, story } => match review_mode(std::env::var("SDC_REVIEW_MODE").ok().as_deref())
            .and_then(|facts| {
                let source = Facts::open(&case_dir);
                if facts { review_inputs(&cwd, &source, &story) } else { review_inputs_session(&cwd, &source, &story) }
            }) {
            Ok(v) => {
                println!("{v}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("sdc-reply review-inputs: {e}");
                ExitCode::from(2)
            }
        },
        Cmd::Adapters { tools } => {
            let (v, refused) = adapters(&tools, &|k| std::env::var(k).ok());
            println!("{v}");
            if refused.is_empty() {
                return ExitCode::SUCCESS;
            }
            for r in refused {
                eprintln!("refused: {r}");
            }
            ExitCode::from(64)
        }
        Cmd::ImplementRun { rec, cwd, case_dir, story, adapter, rework_limit, assemble_only, max_bytes } => {
            let (code, line) = implement::run(&implement::RunArgs { rec, cwd, case_dir, story, adapter, rework_limit, assemble_only, cap: max_bytes });
            println!("{line}");
            ExitCode::from(code)
        }
        Cmd::ImplementPack { tree, note } => {
            println!("{}", implement::pack(&tree, &note));
            ExitCode::SUCCESS
        }
        Cmd::RuleApproval { request, ends } => {
            let (req, ends) = (read_json(&request), read_json(&ends));
            if req.is_null() || ends.is_null() {
                eprintln!("sdc-reply rule-approval: the request or the rule ends are not readable JSON");
                return ExitCode::from(64);
            }
            println!("{}", rule_approval(&req, &ends));
            ExitCode::SUCCESS
        }
        Cmd::CheckCoverage { work, case_dir, story, tree, reply, shape } => match run_check_coverage(work, case_dir, story, &tree, reply, shape) {
            Ok(c) => ExitCode::from(c),
            Err(e) => {
                eprintln!("sdc-reply check-coverage: {e}");
                ExitCode::from(2)
            }
        },
        Cmd::Coverage { work, case_dir, story, schema } => {
            let reqs = story_arg(story).and_then(|s| work_arg(work, case_dir).and_then(|d| story_requirements(&d, &s)));
            match reqs {
                Ok(r) => {
                    let ids: Vec<String> = r.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect();
                    let list: Vec<Value> = r.iter().map(|x| json!({"id": x["id"], "text": x["text"]})).collect();
                    println!("{}", if schema { coverage_schema(&ids) } else { Value::Array(list) });
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-reply coverage: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Cmd::CoverageStub { work, case_dir, story, tree } => {
            match story_arg(story).and_then(|s| work_arg(work, case_dir).and_then(|d| story_requirements(&d, &s))) {
                Ok(r) => {
                    let ids: Vec<String> = r.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect();
                    println!("{}", coverage_stub(&tree, &ids));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("sdc-reply coverage-stub: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Cmd::CheckReview { cwd, case_dir, reply, principal } => {
            let r = read_json(&reply);
            match check_review(&cwd, &Facts::open(&case_dir), &r, &principal) {
                Ok((true, v)) => {
                    println!("{v}");
                    ExitCode::SUCCESS
                }
                Ok((false, v)) => {
                    println!("{v}");
                    ExitCode::from(1)
                }
                Err(why) => {
                    let name = reply.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let what = if r.is_null() { format!("no readable reply ({name}: the adapter's structured output, else out/review.json)") } else { why.join("; ") };
                    eprintln!("invalid review reply: {what}");
                    ExitCode::from(2)
                }
            }
        }
    }
}
