# Layers: Canon, Loom, SDC on Substrate

Question (the operator, 2026-11-20): Canon, Loom and the SDC have blurred layers. (a) What exactly is a
binding, and could Canon ship a default one? (b) One `substrate` layer should decide which outbound
connections are allowed and should start and stop every process, so that everything can run
elsewhere or remotely; each binding implementation runs on the substrate injected into the engine.
(c) Which parts of the code already break these bounds?

Read on 2026-11-20: `sdc/docs/{boundary,driver,loom-sdk-and-agent-node,canon-requests}.md` and
`sdc/driver/src/main.rs` (now 685 lines); substrate `AGENTS.md`, `README.md`, `architecture/
stack-integration.md`, ADRs 0002, 0012, 0013, 0022, 0029, 0036, 0039, 0044, 0047, and
`crates/substrate-host/src/{lib.rs, confinement/{mod,profile}.rs}`; `sandbox/AGENTS.md`;
`secrets/AGENTS.md`; `connectors/{AGENTS.md, README.md, docs/design.md}` and
`crates/connectors-host/src/local/runtime/{launch,process}.rs`; `connectors-definitions/AGENTS.md`;
`slot/AGENTS.md` and `crates/slot/src/{client.rs, server/mod.rs}`; `gates/AGENTS.md` and its
`Command::new` sites; Loom at `abdb0c73` (0.53.0): `AGENTS.md`, `CHANGELOG.md`,
`ess/intake/domains/bindings.yaml`, `crates/loom-commission/src/ports/{effect,connector}.rs`,
`crates/loom-intake-slice/src/{bindings,confinement,effect,git}.rs`, `crates/loom-session-executor/
src/lib.rs` (parts), `crates/loom-connectors/src/cli.rs`, `crates/loom-operator/src/lib.rs`,
`crates/loom-boundary/src/lib.rs`, `crates/loom-sdk/src/lib.rs:95-117`; Canon `AGENTS.md` and
`docs/design/canon-protocol-calculus-design.md`; Atlas ADRs 0082, 0083, 0093, 0095, 0096, 0109,
0110, the decisions of 0023 and 0107, and Atlas's governed-autonomy
incident-response walkthrough. Process starts were found by grepping
`Command::new|process::Command|reqwest` over each repository's `src/`, without tests. Nothing was
built or run. Statements marked *inferred* were not observed in the code or docs.

## Answer in brief

- Most of the substrate the operator describes already exists. Substrate owns process start and stop,
  filesystem scope, egress allow-lists, secret delivery and remote placement, by accepted decision
  (Atlas ADR 0109; substrate ADR 0036:44-50, 0013:20-31, 0012, 0047:80-84). Three things are
  missing. Its consumers have not moved onto it: takt, `sandbox`, Atlas's O6 loop, Loom's bound
  commands, Loom's session executor and Loom's Kubernetes operator. Loom has no single injection
  point for it. SDC bypasses it completely.
- Code and ADRs use "binding" for at least ten different mappings (§ 2). The binding the operator means
  maps an action id to a command, together with a table that turns the result into evidence. Loom
  already ships one as `intake.bindings`. Its commands are argv lists and never shell strings
  (`bindings.yaml:7`), and they run unconfined only when the operator opts in
  (`bindings.yaml:15-18`).
- Canon should ship no executable binding (`canon-protocol-calculus-design.md:1153`: "Canon does
  not execute reasoning or tools"). Canon can ship the pure checks a binding must pass. Its
  abstract "any result this action may produce" binding already exists as the model that `canon
  check` explores (*inferred*).

## 1. What each project is meant to be

| Project | Is meant to be | Must not contain |
|---|---|---|
| Canon | Generic protocol semantics and evaluation with explanation: case, artifact, revision, claim, evidence, obligation, action, authority, outcome (`canon/AGENTS.md:14-16`). Evaluation is a pure function of IR, case, evidence, authority and an explicit instant (`:22-24`). | Engineering vocabulary; any knowledge of AEP, Commission, Loom or a model provider (`AGENTS.md:17-18`); a clock, network, filesystem or credentials in the evaluator (`:23-24`); a workflow engine, scheduler, event store, harness or CI runner (`design.md:1460-1476`); tool execution (`design.md:1153`, "Harness can map admissible actions to tools", `:1172`). |
| Loom | The governed runtime, L3: Commission (case, frontier, authority, effect invocation), the governor, intake, the chooser, loom-sdk (Atlas ADR 0096; `loom/AGENTS.md:26-32`). It makes the effect invocation through the action's binding, while executors only propose (ADR 0082). It resolves capability → contract → connection at run time (ADR 0083). | Protocol semantics, engineering protocols, connector credentials, provider wires, an agent loop (`loom/AGENTS.md:34-41`; ADR 0096). The ADRs give no confinement or placement mechanism to Loom: those belong to Substrate (ADR 0109). |
| SDC | An application on Loom (ADR 0096, "Applications … built on loom-sdk"). It holds the engineering protocols, which belong in engineering-protocols (ADR 0094 via `boundary.md:36`), the action-to-tool data and the exit-code tables (`boundary.md:36-38`). | A runtime of its own: case store, event log, selection or spawn. The driver is "the fourth copy beside Loom, takt and `metaharness aep drive`" (`boundary.md:30-33`). |
| Substrate | The execution data plane: confined workspaces, bounded processes, leases, capsules, a durable operation ledger and observed state. It "runs things and reports what it observed" (`substrate/AGENTS.md:23-25`). It owns confinement, meaning the contract and every adapter (Atlas ADR 0109), and supervised host processes (substrate ADR 0036:44-50). | Product policy, agent loops, connector vendor semantics, the grant engine, identity, model routing, fleet scheduling (`AGENTS.md:199-212`; ADR 0002:17-19). It embeds no sibling component (`AGENTS.md:35-37`). |
| sandbox | A bwrap/`docker run` wrapper "with no daemon, ledger, cgroup bounds or quotas" that copies substrate's posture (`sandbox/AGENTS.md:7-10`). | Anything at all: it is retired into Substrate, and its Docker argv becomes the `oci` adapter (Atlas ADR 0109). |
| secrets | Encrypted byte custody, versions, disclosure, revocation, workload grants, audit (`secrets/AGENTS.md:14-15`; Atlas ADR 0023). | OAuth exchange and refresh: "Provider integrations own" them (`AGENTS.md:16`). Connectors owns subscription-grant custody and refresh (ADR 0107). |
| connectors | Every third-party effect runs "through a declared connection, an admitted operation and an approval" (`connectors/AGENTS.md:7-9`). It holds the connection catalog (ADR 0083) and the credential custody for external calls (Atlas ADR 0110). | Confinement: "Connectors does not reimplement confinement" (ADR 0093). Yet `connectors-host` owns "local supervision" (`docs/design.md:239`) and runs its own credential delivery on fd 3 (`connectors-host/src/local/runtime/launch.rs:1-10`). See § 4. |
| connectors-definitions | Data only: one YAML file per connector, plus auth profiles (`connectors-definitions/AGENTS.md:13-17`). | "No code … no command, binary path or shell string" (`:29`). |
| slot | One build pool per machine: a jobserver, a queue and a live view, plus a remote mode over ssh (`slot/AGENTS.md:3-5`). It "schedules and observes builds" (`:32`). | Build caching or a build system (`:32-33`). It has no confinement of its own; it is a second process funnel beside Substrate (§ 4). |
| gates | Common security checks, receipts, hooks and bot delivery (`gates/AGENTS.md:12`). | Execution of candidate scripts or tools while policy is loaded (`:16`). |

## 2. "Binding": the meanings in use

| # | Binds | Where | Layer |
|---|---|---|---|
| 1 | capability → data contract → Connection, per deployment (`binding/1`) | Atlas ADR 0083; Atlas's incident-response walkthrough | runtime (Loom, ex-Commission), resolved per call |
| 2 | action id → one Connector operation (`instance_id`, `operation_id`) | `loom-commission/src/ports/connector.rs:1-9` (`ActionBinding`) | Loom effect port |
| 3 | action id → argv, run without a shell, with arguments as JSON on stdin and evidence read from the stdout report (`intake.bindings`) | `loom/ess/intake/domains/bindings.yaml:1-21`; `loom-intake-slice/src/bindings.rs:1-18` | Loom effect port (`CommandEffects`) |
| 4 | action id → argv, plus `produces`, `subject`, `producer`, an exit-code → result table, `revises`, `until` and `max_runs` | `sdc/driver/examples/sdc/bindings.json`; `driver.md:83, 149-174` | SDC driver (a fourth runtime) |
| 5 | action id → executor (`program`, `agent` or connector), with inputs, output artifact, budget, sandbox and an evidence table (`loom.bindings/1`, *proposed*) | `loom-sdk-and-agent-node.md:239-283` | Loom (proposal) |
| 6 | Rust core → JavaScript or Python (language bindings) | `loom-sdk-and-agent-node.md:94-98`; `canon-requests.md:50-58` | SDK packaging |
| 7 | evidence record → artifact revision ("revision binding") | `canon/website/docs/reference/evaluation.md:124-127`; `canon/crates/canon/src/eval/binding.rs` | Canon semantics |
| 8 | authority grant → case revision and principal (requested) | `boundary.md:82`; `canon-requests.md:12-16` | Canon semantics (requested) |
| 9 | contract → transport ("Binding"), specification → implementation ("Realization"), custody → platform store | `connectors/docs/design.md:141, 150`; `connectors/README.md:278-281` | Connectors internals |
| 10 | workspace → Git source or root (`GitSourceBinding`, `GitRootBinding`); expression variable → value (`Bindings`) | `substrate-host/src/lib.rs:130, 245`; `canon/docs/design/assertion-api.md:22` | Substrate wire; Canon expressions |

The question uses meanings 3 to 5: **a binding is the operator's trusted declaration of which
executor performs a protocol action, and how its result becomes evidence.** That is what the
binding binds: an action to an executor. Meanings 1 and 2 are the executor-to-connector half for
actions that reach an external system. A binding grants no authority (`bindings.yaml:12-13`), and
no protocol document names a command (`:21`; ADR 0083).

The executor and the substrate are two separate things. A binding names an executor and, at most,
a confinement profile. The engine's configured substrate decides where and how that executor runs
(*proposed*, § 3).

Commands are not free-form shell. Loom runs argv "never through a shell" (`bindings.yaml:7`).
`sh -c` is only a program like any other, and the SDC examples use it
(`examples/sdc/bindings.json:3`). Substrate's exec is argv-only by design (`substrate/AGENTS.md:16`).

Could Canon ship a default binding?

- No executable one: Canon "does not execute reasoning or tools" (`design.md:1153`), and its rules
  forbid IO (`AGENTS.md:22-24`).
- What Canon can ship is pure:
  - Binding checks: produced kinds ⊆ `may_produce` (`boundary.md:76`) and argument validation in
    Canon's dialect (`canon-requests.md:42-44`).
  - The abstract binding "this action may yield any record it `may_produce`". `canon check` already
    explores that model, and `canon simulate` would walk it (`canon-requests.md:29-31`) (*inferred*).
- The default executable binding belongs in Loom. Loom's default for an unbound action is already
  right: the run ends with no local executor and names the action (`bindings.yaml:19-20`).

## 3. The layer stack, bottom to top

Canon is not a layer of this stack. It is a pure library that Loom calls, with no IO and therefore
nothing to confine (`canon/AGENTS.md:22-24`). Connectors, Secrets and Mandate sit beside Substrate,
not above or below it. Each owns one connection (Atlas ADR 0110, proposed; "these components
sit beside the layers", `:85`).

| # | Layer | Owner | Interface to the layer above (cite, or *proposed*) |
|---|---|---|---|
| 0 | Host, cluster, remote worker | Linux bwrap + cgroup, macOS Seatbelt, Docker, Kubernetes; mantle runs a remote daemon (ADR 0109) | `ConfinementAdapter` implementations: `local`, `linux`, `macos`, `systemd`, `oci`, `pod` (ADR 0109) |
| 1 | Substrate | substrate | In process: `substrate_host::confinement::open(AdapterRequest)` → `Arc<dyn ConfinementAdapter>` (`confinement/mod.rs:159`, trait `:723`), admitted against a `ConfinementProfile` (`profile.rs:43`). Over the wire: `substrate-wire/0.26.0` on a Unix socket or HTTPS/WSS through `b10x-substrate-sdk` (ADR 0022; ADR 0029:16-19). On the command line: `b10x-substrate`. |
| 1' | Beside it: Connectors (credentialed external calls), Secrets (custody), Mandate (authority) | connectors, secrets, mandate | Connectors: its CLI `--output json` (`loom-connectors/src/cli.rs:436-445`) behind Loom's `ConnectorInvoker` port (`connector.rs:40`). Topology: an agent confined by Substrate whose only aperture is a Connectors endpoint (ADR 0110). |
| 2 | Executables: tools, harnesses, CLIs (`ess`, `aep`, `b10x-gates`, `worktree`, Claude Code, Codex, b10x-harness via metaharness) | tools (ADR 0095), harness and metaharness (ADR 0096), each tool's repository | argv, exit code and a report on stdout or in a file (`bindings.yaml:7-11`). Tools also get `tool-contract/1` (ADR 0095). |
| 3 | Governed runtime | Loom | Ports: `Governor` (Canon), `EffectPort` (`effect.rs:41-77`), `EvidenceAdapter`, `AuthorityProvider`, `ConnectorInvoker`. Substrate is injected today only through `TestRunner` (`loom-intake-slice/src/confinement.rs:73`), whose implementations are `SubstrateRunner` (`:126`, `run_child` `:475`) and `UnconfinedRunner` (`executor.rs:1163`); `loom-sdk/src/lib.rs:99-117` re-exports them. |
| 4 | Applications | SDC, takt, `b10x-loom` | `protocol/1` (Canon documents), a bindings file (`intake.bindings` today, `loom.bindings/1` *proposed*), loom-sdk |

**Where the substrate is injected (*proposed*).** Inject it once, at Loom's runtime composition
root. The runtime is already generic over the ports a host passes in
(`loom-sdk/src/runtime.rs:30`). The host passes one configured substrate: an opened
`ConfinementAdapter` for embedded use (`mod.rs:159`), or an SDK client for a daemon that is local
or remote (ADR 0022, 0029). Every Loom component that starts a process takes it from there:

- the binding program executor (`CommandEffects`);
- the test runner;
- the session executor's harness spawn;
- the Connectors CLI invoker.

A binding then states only a profile delta: apertures by name, roots, slots. The adapter, the
apertures available and the remote endpoint are engine configuration, never binding data.
Substrate's own rule says the same: "a request selects among what configuration already permitted"
(ADR 0013:22-24).

**What the substrate must expose, and what does it today:**

| Need | Exists in | Gap |
|---|---|---|
| Spawn confined | `ConfinementAdapter::start` (`confinement/mod.rs:757`); the exec route | none |
| Spawn a long-lived, unconfined, supervised process (dev server, daemon) | host-process family: lease, owner, deadline, event stream (substrate ADR 0036:44-50) | none |
| Stop | `signal` (`mod.rs:770`); whole-tree kill (`substrate/AGENTS.md:164`); host process stop by owner and lease expiry (ADR 0036 S4, S5) | none |
| Egress allow-list | apertures declared by the operator and referenced by name, no destination in a request (ADR 0013:20-31); `profile.network` (`profile.rs:77`); socket allowlist (ADR 0039:28-41) | ingress routes and rate bounds are reserved (`profile.rs:94`; ADR 0109) |
| Filesystem scope | workspace, `workspace_access`, writable roots, read-only roots and files, sealed paths (`profile.rs:57-72`) | none |
| Secrets | delivery only: sealed memfd slots (ADR 0012:16-19; `profile.rs:88`). Custody belongs to Secrets and Connectors (Atlas ADR 0023; ADR 0107) | no signer for the grant attribution thread (ADR 0110) |
| Remote placement | Kubernetes namespace driver, chosen by daemon config and never by a request (ADR 0047:80-84); `pod` adapter (ADR 0109); remote SDK (ADR 0029:16-19). Choosing a deployment belongs to "cloud" (`stack-integration.md:37-39`) | nothing in Loom picks a daemon (*inferred*: no SDK dependency in `loom/crates/*/Cargo.toml`) |
| Insecure local mode | `local` adapter, requested with a reason, reported as declared insecure, can be refused by host policy (ADR 0109) | none |
| Audit | `AuditSink` (`mod.rs:353`); operation ledger, durable before dispatch (`AGENTS.md:43-44`) | none |

## 4. Current violations: processes and network calls outside the funnel

"Should go through" names the layer from § 3. Build and release tooling inside each repository
(`connectors-build`, `xtask`, `canon-docs/src/landing.rs:489`) is left out: the engine does not
start those processes for a case.

| # | Where | What | Should go through |
|---|---|---|---|
| 1 | `sdc/driver/src/main.rs:522-524` | `Command::new(argv[0])` per bound action. The environment is inherited (no `env_clear`), there is no timeout, `sh -c` is allowed, and absolute home paths appear in `examples/sdc/bindings.json:3` | Loom binding executor → Substrate. Delete the code once SDC runs on Loom (`boundary.md:28-33`) |
| 2 | `sdc/driver/src/main.rs:180, 339` | spawns the `canon` CLI to evaluate and compile | Canon as a library inside Loom's governor (`boundary.md:67`). A layering error, not a security one |
| 3 | `sdc` bindings `slot ess …`, `slot task check` (`driver.md:161, 169-170`) | Outside a sandbox, the slot daemon runs the job with its own cwd and environment, outside the caller's process tree (`slot/src/server/mod.rs:878-887`). Inside a Substrate sandbox, `socket(AF_UNIX)` is refused (ADR 0039:41), so `slot` runs the job locally (`client.rs:325-328`) and the pool is lost (*inferred*: not run) | a decision: slot jobs as Substrate host processes, or slot as admission only (§ 5) |
| 4 | `loom-intake-slice/src/bindings.rs:415-433` (`CommandEffects`) | operator-bound commands run unconfined behind `--unconfined-bindings` (`bindings.yaml:15-18`; `loom/AGENTS.md:1732-1734`), although `SubstrateRunner::run_child` exists (`confinement.rs:475`) | the injected substrate (`TestRunner` / `run_child`) |
| 5 | `loom-session-executor/src/lib.rs:1390-1396, 2432-2436` and the table at `:15-17` | Claude Code starts through metaharness's `SpawnRunner`, limited by tool policy only. Codex is confined by its own vendor sandbox. Only b10x-harness is confined by Substrate | the injected substrate (ADR 0109: "one consumer path") |
| 6 | `loom-operator/src/lib.rs:4-12`; `loom-boundary/src/lib.rs:1-15`; `loom/CHANGELOG.md:19, 50-53, 203` | Loom runs its own Kubernetes Jobs and pods, its own egress control (NetworkPolicy, loopback relays) and its own secret-file staging | Substrate's namespace driver and `pod` adapter (ADR 0047; ADR 0109), apertures (ADR 0013), secret slots (ADR 0012) |
| 7 | `loom-intake-slice/Cargo.toml:31-32`; `confinement.rs:19-27, 402` | Pins substrate 0.14.0 (substrate is at 0.16.1, wire 0.26.0, `substrate/AGENTS.md:46`). Opens `HostDriver` directly instead of `confinement::open`. Re-declares the profile types in `intake_model` (5,204-line wrapper) | `substrate_host::confinement` (ADR 0109 "loom L1", `:104-108`) |
| 8 | `loom-connectors/src/cli.rs:442`; `loom-plugin/src/effects.rs:1-10` | spawns the `connectors` CLI unconfined | correct owner (Connectors, ADR 0110); the process itself could be a Substrate child with one aperture (*inferred*, low priority) |
| 9 | `loom-protocols/src/git.rs:125`; `loom-intake-slice/src/git.rs:276` | `git` on the host: network fetch of protocol packages, workspace commits | acceptable as trusted host context (`loom/AGENTS.md`: "a remote package never installs executable code"), or Substrate's Git workspace source (`substrate/AGENTS.md:107-108`) (*inferred*) |
| 10 | takt `crates/takt/src/seatbelt.rs:1`; `forge.rs:601-607, 875-877` | its own Seatbelt profiles and `sandbox-exec` | Substrate (ADR 0109; T1, `:107`). Not done |
| 11 | Atlas's O6 loop script; the `sandbox` repository | the O6 loop still calls `b10x-sandbox` | Substrate's CLI; then archive `sandbox` (ADR 0109). Not done |
| 12 | `connectors-host/src/local/runtime/process.rs:330`; `launch.rs:1-10, 416-471` | Its own supervision of adapter processes (parent-death lease) and its own sealed-memfd credential on fd 3 for launched consumers, the same mechanism as substrate ADR 0012 | allowed: no component is required to wrap Connectors (ADR 0110). Still a second process manager and a second secret-delivery path (*inferred*: candidates to compose on Substrate) |
| 13 | `slot/src/server/mod.rs:878-887`; `client.rs:367, 495` | the slot daemon spawns every build, `run_local` spawns directly, and remote mode spawns `ssh` | beside Substrate: slot decides when, Substrate decides how and where (*inferred*, § 5) |
| 14 | `gates/src/delivery.rs:65` (`gh`), `scan.rs:74` (scanners) | network pushes and scanner processes from a tool the SDC binds for `merge` and `push` (`driver.md:173`) | run as a bound tool inside Substrate, with a GitHub aperture and a secret slot for the bot token (*proposed*) |
| 15 | conductor `confine/systemd.rs:64`, autodev (private) | own `systemd-run` and bwrap | Substrate, in later hops (ADR 0109). Cited from the ADR, not re-read |

Canon: no process start and no network call in `canon/crates/*/src` outside documentation tooling
(grep, 2026-11-20). Canon is clean.

## 5. Recommendation

1. **Canon** keeps semantics only and ships no executable binding. It may ship pure binding checks
   (`may_produce` coverage, argument validation) and the abstract "any producible result" model that
   check and simulate use (`design.md:1153`; `canon-requests.md:42-44`).
2. **Substrate** owns process start and stop, filesystem scope, egress apertures, secret delivery
   and remote placement. This is decided (ADR 0109; substrate ADR 0036, 0013, 0012, 0047), so
   no new repository and no new layer is needed.
3. **Connectors** owns credentialed calls to external systems and the action → operation half of a
   binding. **Secrets** owns custody. **Mandate** owns approval (ADR 0110).
4. **Loom** owns the one binding format and every binding executor (`program`, `agent`,
   `connector`). It receives the substrate by injection at its composition root and owns no
   confinement or Kubernetes mechanism of its own (violations 4 to 7).
5. **SDC** owns data only: protocols (moving to engineering-protocols), a bindings file and result
   tables. It owns no spawn code.
6. **slot** stays a scheduler beside Substrate. Inside a sandbox, bindings call the build program
   directly, and the engine applies the pool limit when it admits the action (*inferred*). The
   alternative is slot running jobs as Substrate host processes (ADR 0036). That is slot's decision
   to make, not Loom's.
7. Archive `sandbox` and finish takt T1, as ADR 0109 already decides (violations 10 and 11).

Top three changes, by value per cost:

8. **Loom: run bound commands through the injected substrate.** `CommandEffects::run`
   (`bindings.rs:415`) calls `SubstrateRunner::run_child` (`confinement.rs:475`), and
   `--unconfined-bindings` becomes the `local` adapter with a reason (ADR 0109). Value: this
   is the one path where Loom already runs operator commands for a case outside Substrate, and it is
   the path the SDC needs. Cost: small. The runner, its refusals and `ChildCommand` exist and are
   tested (`loom/AGENTS.md:1740-1745, 1759-1766`), and loom-sdk already exports them
   (`loom-sdk/src/lib.rs:99-117`).
9. **Loom: extend `intake.bindings` into `loom.bindings/1`**, adding the result table, `revises`,
   the executor kind and a profile delta, so that `sdc/driver/examples/sdc/bindings.json` becomes a
   Loom file and `sdc-drive` loses its spawn (`main.rs:522`). Value: it removes the fourth runtime
   copy (`boundary.md:30-33`), and the SDC gets confinement, timeouts and a cleared environment for
   free (violation 1). Cost: moderate. The format and its ESS domain exist (`bindings.yaml`); result
   tables are new (`loom-sdk-and-agent-node.md:73`). Do it after change 8, so the format is born
   confined.
10. **Loom: repin substrate and open adapters through `confinement::open`** (`mod.rs:159`) from
    engine configuration, instead of opening `HostDriver` directly (`confinement.rs:402`). Value:
    this makes "the engine is configured with a substrate" literal. `local`, `linux`, `macos`,
    `oci` and `pod` become one setting, and remote placement comes for free through `pod` or the
    SDK (ADR 0029). Cost: one pin hop plus the "loom L1" step that ADR 0109 already plans
    (`:104-108`).
11. Next, at a higher cost: move `loom-operator`'s Jobs, NetworkPolicy and secret staging onto
    Substrate's namespace driver and `pod` adapter (violation 6). This is the largest duplicate.

Where I disagree with the framing:

12. Substrate does not "define" outbound connections. It enforces the allow-list that operator
    configuration declares (ADR 0013:22-24). Whether a credentialed call to an external system is
    allowed is Connectors' decision under a grant (ADR 0110; ADR 0002:17-23). Bindings should
    be able to select apertures by name, never declare them.
13. "Every process through one process manager" is right for what the engine starts for a case.
    But Substrate already has two families with different guarantees: confined exec (ADR 0004) and
    unconfined supervised host processes (ADR 0036:44-50). One funnel does not mean one profile.
    slot is a scheduler, not a lifecycle owner (`slot/AGENTS.md:32`). It stays beside Substrate,
    not inside it.
14. Substrate is not a layer under the bindings in ADR 0096's stack. Atlas places it beside the
    layers (ADR 0110, still *proposed*), and Canon does not sit on it at all. The useful form of
    the operator's idea is a port injected into Loom, not a new layer in the stack table.
15. A binding's command is an argv list, not a free-form shell string (`bindings.yaml:7`;
    `substrate/AGENTS.md:16`). A shell is allowed only as a declared program, and it then makes the
    command harder to audit (*inferred*).

## Not verified

- metaharness's own spawn path (`SpawnRunner`) and whether metaharness can take a Substrate runner.
- conductor and autodev confinement sites: cited from ADR 0109, not re-read.
- Whether Loom's or Substrate's planning stores already hold stories for changes 8 to 10.
- What slot does under a Substrate sandbox (violation 3): reasoned from ADR 0039 and `client.rs`,
  not run.
