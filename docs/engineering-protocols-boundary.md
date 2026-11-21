# engineering-protocols: catalog or crates, and the plan for a data-only catalog

Written 2026-11-21, read-only research. Paths: EP = `../engineering-protocols`, L = `../loom`, T = `../takt` (relative to this repository). "Inferred" marks a statement no source states. No build or test was run.

## Decision (operator, 2026-11-21, relayed by the coordinator, answer "A")

engineering-protocols becomes a data-only catalog. Loom loads protocols from files, observation collection moves into Loom executors through Substrate, the crates shrink to a check run in CI, and a Canon bump then needs no engineering-protocols release. In his words: "engineering protocols was just that, a collection of potential flows, and then there is an example implementation for the binding, so we could demonstrate it, but maybe this is now superseded anyway." This replaces ADR 0094 decision 2 ("a runtime pins a release of `b10x-canon-engineering` and reads protocols through its registry", Atlas ADR 0094). Its basis line already says runtimes "would simple embed them or 'vendor' them".

## 1. The four crates (EP/crates)

Workspace version 0.16.0 (`EP/Cargo.toml:7`). Every crate pins Canon by git tag 0.9.0; `EP/Cargo.lock:108,150` resolves it to commit 8f1f4527. Line counts are `cat` of `.rs` files; "tests" includes `tests/support` and fixtures `mod.rs`.

| crate | does | public surface | reads from disk | processes / network |
|---|---|---|---|---|
| `canon-engineering` (src 911, tests 20,282, `build.rs` 85) | Embeds every `protocols/<name>/<major>.yaml` at build time (`build.rs:44-85`); reads `vocabulary.yaml`. Binary lists/shows protocols and hosts the `gates` and `assertions` commands. | `registry::{list,get}` (`src/registry.rs:83,92`), `registry::Builtin{name,major,yaml,model}`, `vocabulary::*` (586 lines), four id constants (`src/lib.rs`) | `protocols/` and `vocabulary.yaml` at build time only (`build.rs`, `vocabulary.rs:39`); nothing at run time | none in src. Tests spawn its own binary, `cargo` and `git` (`tests/assertion_replay.rs:43,212`, `crate_names.rs:52`) |
| `canon-engineering-assertions` (src 1,334, tests 1,937) | Typed engineering gates: plans assertions with Canon's expression crate, collects observations, retains evidence, replays offline. Catalog of 26 functions in `EP/catalog/engineering.yaml` (926 lines). | `prepare`, `prepare_with`, `collect`, `evaluate`, `evaluate_gates`, `Bundle`, `GatesReport`, `builtin_catalog` (`src/lib.rs:163,332,348,623,754,780`), `cli::{Gates,Assertions}` | gates and project documents (4 MiB cap, `lib.rs:21,177`); whole source tree for the identity hash, 1 GiB cap (`snapshot.rs:11-104`) | none itself; calls `assertion_providers::collect` (`lib.rs:~650`). Reads the clock in the CLI (`cli.rs:189,201`) |
| `assertion-providers` (src 1,940, tests 1,710, generated 397) | Collectors: files, git, process, network, reports. Resolves and digests programs, runs them with cleared env in one process group. | `collect`, `ProviderContext`, `CommandBinding`, `admit_command`, `resolve_program`, `resolve_change` (`src/lib.rs:8-9,152`) | files below the root (`files.rs:50,207,229`), reports | spawns programs: `process.rs:171,184` (`Command::new`, `process_group(0)`), `libc::kill`/`fcntl` (`:97,114`); git via the same runner (`git.rs:203-230`); TCP connect `network.rs:28`, UDP DNS `network.rs:59`. Its README says "resource supervision, not an operating-system sandbox; registered programs possess the caller's authority" (`README.md:17-18`) |
| `canon-engineering-docs` (src 3,186, tests 2,840, `publish = false`) | Generates protocol pages, graphs, vocabulary page, status, site manifest into `EP/website` (`src/main.rs:20-60`); `generate --check` is a CI step. | CLI only (`generate`, `site-manifest`) | `protocols/`, `vocabulary.yaml`, `website/` (`generate.rs:52-102`) | none ("nothing here touches the network", `main.rs:5`) |

Dependency chain (Cargo.toml of each): `canon-engineering` -> `assertions` -> `assertion-providers`. A dependent that only calls `registry::get` therefore also compiles the process-spawning and network code (`canon-engineering/Cargo.toml:17`; `loom/Cargo.lock:237-265` lists all three). `catalog/` is the assertion function catalog (`canon-catalog/1`), not a catalog of protocols.

Why 20,282 test lines for 911 source lines: the tests are the protocols' tests, not the crate's. 63 files, 19,062 lines, 247 `#[test]`; 28 files / 8,509 lines are `adversary*` passes, 1,892 are `story_delivery*`, 1,040 the fixture harness (`tests/support/mod.rs`). They evaluate the 10 YAML protocols against 115 fixture files / 10,308 lines under `EP/fixtures` through Canon, and model-check each (`builtin_canon_check.rs`). Rust is the harness. 709 lines test the registry, names, vocabulary coverage (inferred from file names and the AGENTS.md invariants table, `EP/AGENTS.md:57-77`). Only the registry/vocabulary part tests the crates.

The repo holds 10 `protocol/1` documents: eval-gated-change/1, incident-response/1, loop-improvement/1, ml-experiment/1, protocol-adopt/1, self-improvement/1, software-change/1, story-delivery/1 and /2, support-triage/1 (`README.md:30-41`), plus `protocols/vocabulary.yaml`.

## 2. Dependents (grep of every `Cargo.toml` and `Cargo.lock` under the workspace root `..`)

No match in `metaharness`, `aep`, `conductor`, `sdc` or `atlas` (Cargo files). Atlas's catalog lists the relations ep-loom, ep-takt, ep-examples, which would need an update (inferred).

| dependent | pins | direct or via | state |
|---|---|---|---|
| loom `b10x-loom-protocols`, `-governor`, `-intake-slice`, `-intake-router` (`L/crates/*/Cargo.toml:15,19,18,15`) | ep tag 0.15.2, Canon 0.8.0 | direct; lock also holds assertions and providers (`L/Cargo.lock:211-265`) | Loom 0.57.2, 2026-11-05 |
| takt `b10x-takt`, `takt-cli` (`T/Cargo.toml:12-15`) | ep tag 0.9.0, Canon 0.6.0 | direct, incl. assertions | last commit 2026-10-23; 16 commits after tag 0.9.0 |
| a private consumer, control-plane, loom-coder, examples/zendesk-triage | ep 0.7.0 (the private consumer; `control-plane/Cargo.lock:210`), 0.1.0 (`loom-coder/Cargo.lock:144`), repo `els` rev 59fd19f (`examples/zendesk-triage/Cargo.toml:20`) | through an old Loom pin; examples directly | old pins, never follow ep HEAD |

Call sites (file:line). Needs: LOOKUP = `registry::get(name, major)`, LIST = `registry::list`, EVAL = assertions. Loom has 15 of the 18 lookups, takt 3. All 7 assertion mentions are takt's (one is a comment). Loom calls none.

| need | production sites | test sites |
|---|---|---|
| LOOKUP (11) | `L/crates/loom-governor/src/lib.rs:1631` (fallback `builtin()`, used at `:910,:1598`); `L/crates/loom-protocols/src/lib.rs:162`; `T/crates/takt/src/protocol.rs:60,61` | `L/.../loom-governor/tests/governed_case.rs:613,621`, `adversary_governor.rs:1044`, `adversary2_governor.rs:643`; `loom-intake-slice/tests/adversary_case.rs:248`; `loom-intake-router/tests/adversary_classify.rs:477`, `classify.rs:146` |
| LIST (7) | `L/crates/loom-protocols/src/lib.rs:160`; `T/crates/takt/src/protocol.rs:297` (also `takt-cli/src/main.rs:801,2551`, wrapper) | `adversary_case.rs:244`; `adversary_classify.rs:116,476`; `classify.rs:91,145` |
| public type leak | `L/crates/loom-intake-router/src/classify.rs:32` `RouterError::Registry(canon_engineering::registry::Error)`, matched at `L/crates/loom-plugin/src/turn.rs:282`; `classify` builds `ProtocolCatalog::engineering()` at `:108` | |
| EVAL: prepare (2) | `T/crates/takt/src/validator.rs:1987` | `T/crates/takt-cli/tests/takt_gates_document.rs:132` |
| EVAL: collect (1) | `T/crates/takt/src/validator.rs:2030` | |
| EVAL: evaluate (3, one a comment) | `validator.rs:2039` | `T/crates/takt/tests/gate_validator.rs:1091`; comment `tests/no_own_evaluation.rs:231` |
| EVAL: `Bundle` (1) | | `gate_validator.rs:1088` |

Only takt runs observation collection in production. It must work around the library: "The library starts the commands and providers the project file binds itself, outside `crate::confinement::Command`", so takt asks its adapter first (`validator.rs:2013-2029`).

## 3. What serves each need without a compile-time dependency

- LOOKUP / LIST. Loom's registry is already `ProtocolCatalog`, keyed `name@major` (`L/crates/loom-protocols/src/lib.rs:86-175`); `loom.bindings/1`'s `protocol` is such a registration. It loads protocols from text, not only from the crate: `add_yaml` (`:182`) validates, compiles and records a sha256 of the bytes (`:210`); `InstallStore` installs from a local file (`store.rs:62`) or a pinned 40-hex git commit (`store.rs:87`, `git.rs:6-30`) into `$XDG_DATA_HOME/loom/protocols`; the governor admits an exact catalog by digest (`loom-governor/src/lib.rs:766-830`). Only the bundled seed (`engineering()`, `:158-174`) and the governor fallback come from the crate. Note: `loom.bindings/1` itself is not in Loom main (grep empty; SDC waits on it: `sdc/.engineering/planning/upstream-blocker/loom-bindings-document.md`).
- Digest. Loom hashes document bytes. Canon's `--import` pins revision plus sha256 of compiled `canon-ir/1`, but `story:protocol-imports` is a draft (`canon/.engineering/planning/story/protocol-imports.md`, revision 9) and no `--import` exists in `canon-cli` 0.9.0 (grep empty). Compiled-IR digests also change with Canon's IR; source-byte digests do not (inferred).
- Canon version. A protocol file can need a newer Canon than the one a runtime links: `story.delivery/2` uses Canon 0.7.0's `after` and 0.8.0's `since` (`CHANGELOG.md:95-114,69-93`). The catalog must say which Canon each file needs (not recorded today; unknown how Canon reports it).
- EVAL (collect, prepare). Observations as Loom executors through Substrate: Loom already has `TestRunner` and `SubstrateRunner` (`L/crates/loom-intake-slice/src/confinement.rs:73,126`) that run `tests.run` confined, and Substrate owns exec and egress apertures (`substrate/README.md:413-428`, ADR 0109 "all the sandboxing code ... should live actually in substrate"). No executor for `git.changed`, file/document checks, report readers or DNS/TCP exists there today (inferred from `crates/` listing; not read in full).
- Docs. A generator that reads the files and links Canon as a library (it uses `b10x_canon::{ir,model}`, `generate.rs:10-11`); it needs no registry. It does import the assertions crate for the reference page (`generate.rs:238`, `main.rs:55`).

## 4. Overlaps

assertion-providers against Substrate and Loom: `process.run`, `tests.run`, `ess.run`, `git.*` and `net.*` start programs and open sockets inside the caller with the caller's authority and an unsandboxed process group (`README.md`, `process.rs:171-184`). Substrate's contract is confinement plus egress apertures with observed facts. Loom's runner records the confinement it proved. takt's wrapper (above) is the symptom. Providers that only read files and reports (`files.rs`, `reports.rs`, 248 + 284 lines) have no Substrate counterpart; they need no confinement beyond a read-only workspace (inferred). DNS/TCP: Substrate's egress aperture admits one `host:port/tcp`; a DNS-over-UDP observation has no named equivalent (unknown).

`story.delivery/2` and `software.change/1` against SDC's eight (`sdc/protocol/*.protocol.yaml`: sdc/4, sdc.intake/6, sdc.direction/4, sdc.epic/2, sdc.spec/4, sdc.plan/5, sdc.story/4, sdc.release/3; `sdc` has no commits yet, so none is released). Names only:

| | story.delivery/2 | software.change/1 | sdc.story / sdc.release |
|---|---|---|---|
| shared | claims `tests.pass`; actions `tests.write`... `tests.run`, `repository.inspect` | `tests.pass`, `tests.run`, `repository.merge`, `release.rollback`; evidence `code_review`, `build_provenance`, `rollback_result`, `implementation_change` | `tests.pass`, `tests.run`; `implementation_change`, `code_review`, `build_provenance`, `rollback_result`, action `release.rollback` |
| only ep | `tests_first`/`gate_on` inputs, `gate.run`, `pr.open`, `ci.read`, red-first claim `tests.shown_red`, `merge_observation`; outcomes accepted/abandoned | security review, independence dims, risk profiles, deployment/health, outcomes incl. declined/superseded/rolled_back | |
| only SDC | | | ESS gate (`ess_gate`, `spec.gate`), `conformance_run`, findings loop (`amend.upstream`), spec/plan/epic/intake/direction stages, child cases (`case_outcome`, `*.open`), `release_conformance`, `withheld` |

Both families are disjoint apart from the shared names; neither could replace the other (inferred from the name lists). `sdc.story` has no red-first or forge steps; `story.delivery/2` has no spec-revision gate. `sdc.release` overlaps `software.change/1`'s release half only by names.

## 5. What is lost if the crates go, and where it lives

| lost | where instead |
|---|---|
| Offline replay of assertions (`evaluate`, pure, `lib.rs:754`; takt `gate_validator.rs:1088-1091` replays two retained bundles) | with the collector: wherever `engineering-gates/1` execution lands. Owner unknown (Loom, takt, or a tool on Canon's expression crate) |
| The vocabulary check (`vocabulary.rs`, 586 lines; coverage test) | a CI check tool in EP reading `protocols/vocabulary.yaml` and the protocols; no consumer needs it at run time (grep: no dependent calls `vocabulary::`) |
| The docs site (`generate --check`, `pages.yml`) | the docs generator stays in EP as a CI/dev tool; its assertion reference page goes with the assertions or is dropped |
| Conformance tests (20,282 lines; fixtures) | stay in EP: they test the catalog, not the crates. They need a Canon library, so a CI-only crate remains (`publish = false`) |
| `canon-engineering protocols list/show` binary | files plus `ls`; or kept in the check tool (121 lines) |
| `gates validate/run/explain` CLI | with the collector (unknown owner) |

## 6. The coupling cost today

22 release tags (0.1.0 to 0.16.0). Canon pin moved in 8 of them: 0.2.0, 0.4.0, 0.6.0, 0.7.0, 0.8.0, 0.13.0, 0.14.0, 0.16.0 (`git show <tag>:Cargo.lock`).
- Only a Canon bump: 0.8.0 (Canon 0.4.0 to 0.6.0; "no protocol here declares either", `CHANGELOG.md:317-327`) and 0.16.0 (Canon 0.9.0; "no protocol, fixture verdict or generated file changes", `CHANGELOG.md:9-26`, commit 5d55ece).
- Bump plus a protocol rule that needs the new Canon form: 0.6.0 (`first/before`), 0.7.0 (independence), 0.13.0 (`after`), 0.14.0 (`since`). Here the data itself needs the Canon version; a data-only catalog still needs a "requires Canon >= x" field.
- Breaking Rust API releases with no protocol change: 0.10.0 (`Marking` variants), 0.11.0, 0.12.0 (`CommandBinding` fields) (`CHANGELOG.md:150,178,242`). What waited: commit 5d55ece says "registry::get returns Canon's model type, so a consumer moves to Canon 0.9.0 with this release. Loom needs it: three Loom tests refuse a second Canon copy." Loom's rule is one Canon, named by the ep crate's reference (`L/AGENTS.md:506-509`); Loom still pins Canon 0.8.0 and ep 0.15.2 (`L/crates/loom-governor/Cargo.toml:15,19`). Gap from Canon tag to ep tag: 0.6.0 to ep 0.8.0 about 29 h; 0.7.0 to 0.13.0 about 1.6 h; 0.8.0 to 0.14.0 about 0.7 h; 0.9.0 (2026-10-30 00:55) to 0.16.0 (2026-11-21 02:07) about 22 days (tag dates, `git for-each-ref`). Why the last took 22 days is not recorded; Loom moved with each earlier pair on the same day (`L/CHANGELOG.md:209,225`). Note: after option A Loom still links Canon directly (`ProtocolEntry::model` is a Canon type), so a Canon bump is still a Loom release, but no longer waits on a second repository.

## 7. Migration (option A), in order

"Breaks" = breaks a released consumer. Every released consumer pins a git tag or lock commit (section 2), so removing code at EP HEAD cannot break one while the tags stay (assumption: no repository delete or force-push; the `els` rename needs a redirect, unknown).

| # | step | owner | breaks a released consumer? |
|---|---|---|---|
| 1 | Publish the catalog as data: a manifest (`protocols/manifest.yaml`, inferred name) with path, id, revision, sha256 of each file, the Canon version each needs, and a release archive plus digest per tag. CI checks the manifest against the files. | engineering-protocols | no, additive |
| 2 | Loom loads the catalog from files: vendored `loom/protocols/engineering/` plus a digest lock; `ProtocolCatalog::engineering()` and the governor fallback read it; drop `b10x-canon-engineering` from the four crates; 12 test sites read the same files; Canon stays pinned by Loom. `RouterError::Registry` goes (public, `classify.rs:32`). | loom | no for old tags; a Loom Breaking entry for the router type. Can ship alone once step 1 exists |
| 3 | takt: `protocol.rs:60,61,297` read vendored files (takt already embeds its own, `protocol.rs:33`); keep or drop takt's assertions use per step 5. Optional: takt is pinned to ep 0.9.0 and idle since 2026-10-23 (its `story.delivery/2` is not in 0.9.0). | takt | no |
| 4 | Move observation collection behind Loom executors through Substrate: requires a design of what `engineering-gates/1` and the 26-function catalog become, which providers Substrate exec/aperture can serve, and where replay lives. Not started. | loom + substrate (design: operator) | no, new capability; the crates stay until it lands |
| 5 | takt moves `prepare/collect/evaluate` (`validator.rs:1987-2039`) to that executor, or keeps its pin at 0.9.0. | takt | no |
| 6 | Delete `canon-engineering` (registry, `build.rs`, vocabulary reader) and then assertions and providers; keep a CI check (vocabulary, `canon check` of every protocol, fixtures) and the docs generator, all `publish = false`, nothing depends on them. Delete the stale AGENTS.md lines (`AGENTS.md:39,51`, "tag 0.1.0") and the README install section (`README.md:44`). | engineering-protocols | no (tags stay); step 6 for assertions only after 4 and 5, or freeze them as "unmaintained at tag 0.16.0" |
| 7 | Supersede Atlas ADR 0094 decision 2; update catalog relations ep-loom, ep-takt, ep-examples. | atlas | no |
| 8 | SDC's eight protocols join the catalog when stable. They are unreleased (no commits), use terms outside the vocabulary (`ess_gate`, `spec.gate`), and the ep vocabulary check would need markings or an exemption (as `support.triage/1`, `AGENTS.md:19-23`). Layers doc already says "moving to engineering-protocols" (`sdc/docs/layers.md:187`). | sdc then ep | no |

Unknown: owner and shape of step 4; whether anything runs `canon-engineering gates` outside takt (`todo-app-example/AGENTS.md:8` mentions a gate file); the cause of the 22-day gap; whether Canon's `--import` will replace Loom's byte digest; worktrees were not scanned for other pins.

## Requests ready to send

For the `loom` session. Drop the compile-time dependency on engineering-protocols (operator decision A, 2026-11-21). Today: `b10x-canon-engineering` tag 0.15.2 at `crates/loom-protocols/Cargo.toml:15`, `loom-governor:19`, `loom-intake-slice:18`, `loom-intake-router:15`. Wanted: load the catalog from files and check it by sha256, then delete those four lines. Production sites: `loom-protocols/src/lib.rs:160,162` (`engineering()`), `loom-governor/src/lib.rs:1631` (`builtin()`, used `:910,:1598`), `loom-intake-router/src/classify.rs:32,108` (`RouterError::Registry`, matched `loom-plugin/src/turn.rs:282`). Test sites (12): `governed_case.rs:613,621`, `adversary_governor.rs:1044`, `adversary2_governor.rs:643`, `adversary_case.rs:244,248`, `adversary_classify.rs:116,476,477`, `classify.rs:91,145,146`. Suggested path: vendor `protocols/engineering/` with a digest lock; `ProtocolCatalog::add_yaml` and `InstallStore` (`store.rs:62,87`) already do the rest. Loom has none of the 7 assertion calls (all takt's), so nothing there moves. Keep Canon pinned by Loom. Ships without breaking a released consumer; the router error type change is a Loom Breaking entry. The manifest format comes from the ep session; take the files from tag 0.16.0 until then.

For the `engineering-protocols` session. Make the repository a data-only catalog (operator decision A, 2026-11-21). (1) Add a manifest: path, id, revision, sha256 of each of the 10 protocols and `vocabulary.yaml`, plus the Canon version each file needs (`story.delivery/2` needs >= 0.8.0, `CHANGELOG.md:69-93`), and a release archive with its digest; check it in CI. Rename or move `catalog/engineering.yaml` (it is the assertion function catalog, not protocols). (2) Shrink the crates: after Loom (`loom-protocols`, `-governor`, `-intake-slice`, `-intake-router`) and takt stop depending (`takt/Cargo.toml:13-14`), delete `canon-engineering` and keep one `publish = false` check tool: vocabulary check, `canon check` of every protocol, the 63 test files and 115 fixtures. (3) The docs generator (`canon-engineering-docs`) stays; it reads files and links Canon (`generate.rs:10-11`); its assertion reference page (`generate.rs:238`) goes with the assertions or is dropped. (4) Offline replay (`evaluate`, `lib.rs:754`) and the collectors (`process.rs:171`, `network.rs:28,59`) are not deleted until the Loom/Substrate executor exists (owner unknown); freeze them at tag 0.16.0 meanwhile. (5) Fix `AGENTS.md:39,51`. No released consumer breaks: all pin tags.

## Verdict

1. Operator's view: mostly right. The catalog is data, and nothing needs to compile against it: Loom already loads protocols from text with digests.
2. The crates are the harness for the protocols' tests (20,282 test lines against 911 source lines), not the registry, so the tests stay; only the registry crate is redundant.
3. The compile-time coupling is real: three Canon-only releases and a 22-day wait for 0.16.0, plus a public Loom error type that leaks the ep crate.
4. Partly wrong: a catalog still depends on Canon's version (four releases ship rules that need a newer Canon), and the Rust that touches the world has no home yet; only `tests.run` has a Loom/Substrate executor, so assertions cannot be deleted until step 4.
5. Evidence: sections 1 to 6; least certain are the owner of step 4, the cause of the 22-day gap, and whether anything but takt runs `gates`.
