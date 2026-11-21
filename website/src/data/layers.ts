/**
 * The layers and components Runbook is built on, with what each supplies today. The layering is
 * `docs/layers.md` § 3; every statement cites this repository at tag 0.1.1 or the component's
 * public README. Written by hand: these are claims about other repositories, not generated data.
 */

export type Use = 'used' | 'partly' | 'not-yet' | 'planned';

export interface Cite {
  label: string;
  href: string;
}

export interface Component {
  id: string;
  name: string;
  repo: string;
  site?: string;
  is: string;
  isCite: Cite;
  use: Use;
  supplies: {text: string; cite: Cite}[];
  temporary?: {text: string; cite: Cite}[];
}

export interface Layer {
  n: string;
  name: string;
  members: string[];
  note?: string;
}

const sdc = (path: string, lines?: string): Cite => ({
  label: lines ? `${path}:${lines}` : path,
  href: `https://github.com/beyond10x/sdc/blob/0.1.1/${path}${lines ? `#L${lines.replace('-', '-L')}` : ''}`,
});

const readme = (repo: string, lines: string): Cite => ({
  label: `${repo} README.md:${lines}`,
  href: `https://github.com/beyond10x/${repo}#readme`,
});

export const USE_LABEL: Record<Use, string> = {
  used: 'Used today',
  partly: 'Used in part',
  'not-yet': 'Not used yet',
  planned: 'Planned',
};

export const LAYERS: Layer[] = [
  {n: '4', name: 'Applications', members: ['sdc'], note: 'protocol/1 documents, a bindings file, the runtime\'s SDK'},
  {n: '3', name: 'Governed runtime', members: ['loom']},
  {n: '2', name: 'Executables: tools and CLIs', members: ['ess', 'aep', 'gates', 'worktree', 'slot']},
  {n: '1', name: 'Execution data plane', members: ['substrate'], note: 'beside it: Connectors, Secrets, Mandate'},
  {n: '0', name: 'Host, cluster, remote worker', members: [], note: 'Linux bwrap and cgroups, macOS Seatbelt, Docker, Kubernetes'},
];

export const KERNEL = 'canon';

export const COMPONENTS: Component[] = [
  {
    id: 'sdc',
    name: 'Runbook',
    repo: 'https://github.com/beyond10x/sdc',
    is: 'An application on Loom. It holds the protocols, the action-to-tool data and the exit-code tables; it must not hold a runtime of its own.',
    isCite: sdc('docs/layers.md', '51'),
    use: 'used',
    supplies: [
      {text: 'Nine protocol/1 documents, one bindings file per protocol and the tools they bind.', cite: sdc('AGENTS.md', '18-20')},
      {text: 'The driver sdc-drive, which today also holds the runtime parts listed under Loom.', cite: sdc('docs/driver.md', '42-45')},
    ],
  },
  {
    id: 'canon',
    name: 'Canon',
    repo: 'https://github.com/beyond10x/canon',
    site: 'https://beyond10x.github.io/canon/',
    is: 'A formal language and deterministic calculus for evidence-governed protocols.',
    isCite: readme('canon', '3'),
    use: 'used',
    supplies: [
      {text: 'The protocol/1 semantics every SDC protocol is written in, and its evaluation: for each case, the claims, the open obligations, the admissible actions and the legitimate outcomes.', cite: sdc('docs/protocol-notes.md', '18-20')},
      {text: 'canon validate, compile and check on all nine protocols, and the ESS gate property proved with probe copies.', cite: sdc('scripts/check-protocols.sh', '1-6')},
      {text: 'canon simulate, pinned at release 0.10.0 (commit 2c7a1ea6); each report is compared with the committed one.', cite: sdc('docs/releases/0.1.1.md', '23-26')},
    ],
    temporary: [
      {text: 'The driver calls canon evaluate as a subprocess and keeps the case as plain files; Canon is meant to be a library inside Loom\'s governor.', cite: sdc('docs/driver.md', '50-51')},
    ],
  },
  {
    id: 'loom',
    name: 'Loom',
    repo: 'https://github.com/beyond10x/loom',
    site: 'https://beyond10x.github.io/loom/',
    is: 'The agent runtime: it runs governed agents, and b10x-loom-sdk is its library for building on it.',
    isCite: readme('loom', '3-5'),
    use: 'not-yet',
    supplies: [
      {text: 'Not used yet. Runbook is meant to run as an application on Loom.', cite: sdc('docs/layers.md', '51')},
    ],
    temporary: [
      {text: 'Child cases (case_open to case_outcome) and the roadmap.read copy are built in the driver as a temporary copy of what Loom must provide, marked TEMPORARY in driver/src/main.rs.', cite: sdc('docs/driver.md', '42-45')},
    ],
  },
  {
    id: 'substrate',
    name: 'Substrate',
    repo: 'https://github.com/beyond10x/substrate',
    site: 'https://beyond10x.github.io/substrate/',
    is: 'The execution data plane: confined workspaces, bounded processes and sessions, durable operations, leases and observed state.',
    isCite: readme('substrate', '3-4'),
    use: 'not-yet',
    supplies: [
      {text: 'Not used yet: Runbook bypasses it completely. Confinement and process start are meant to reach Runbook through Loom.', cite: sdc('docs/layers.md', '33-34')},
    ],
  },
  {
    id: 'ess',
    name: 'ESS',
    repo: 'https://github.com/beyond10x/ess',
    site: 'https://beyond10x.github.io/ess/',
    is: 'Describes systems as typed data, compiles them into deterministic IR, projects artifacts and checks implementations against what was declared.',
    isCite: readme('ess', '3-5'),
    use: 'used',
    supplies: [
      {text: 'The specification format of every epic, and the spec gate: validate, compile, conformance synthesis with no refusal and no open-question marker.', cite: sdc('docs/phases.md', '34-35')},
      {text: 'Conformance runs as the verification evidence of a story and a release (tests.run, acceptance.run).', cite: sdc('docs/driver.md', '715')},
      {text: 'The specification of Runbook\'s own decision contract (domain sdc.decision).', cite: sdc('AGENTS.md', '25')},
    ],
  },
  {
    id: 'aep',
    name: 'AEP',
    repo: 'https://github.com/beyond10x/aep',
    site: 'https://beyond10x.github.io/aep/',
    is: 'Keeps a repository\'s engineering plan as Markdown files that a program governs.',
    isCite: readme('aep', '3-6'),
    use: 'used',
    supplies: [
      {text: 'Inside a run: the vision, roadmap and stories; the model writes a proposal and the tool makes the aep plan artifact calls.', cite: sdc('docs/driver.md', '359-361')},
      {text: 'For this repository: the planning store in .engineering/planning/, changed only through the aep CLI.', cite: sdc('AGENTS.md', '45-46')},
    ],
  },
  {
    id: 'gates',
    name: 'Gates',
    repo: 'https://github.com/beyond10x/gates',
    is: 'Common security and privacy checks, signed local evidence, and bot delivery for beyond10x repositories.',
    isCite: readme('gates', '3'),
    use: 'partly',
    supplies: [
      {text: 'For this repository: every commit and push is the bot\'s, made with b10x-gates bot, and CI runs the shared Gates workflow pinned by commit.', cite: sdc('AGENTS.md', '76-84')},
      {text: 'In a run: not used yet; the bound tools make no b10x-gates call (inferred from the tool script).', cite: sdc('driver/examples/sdc/tools/sdc-tool.sh')},
    ],
  },
  {
    id: 'worktree',
    name: 'worktree',
    repo: 'https://github.com/beyond10x/worktree',
    is: 'One safe lifecycle for Git worktrees, with leases, recovery proof and safe cleanup.',
    isCite: readme('worktree', '3-5'),
    use: 'planned',
    supplies: [
      {text: 'Planned: each admitted child case gets its own managed worktree. Today a case\'s work/ is a plain directory (docs/driver.md:198).', cite: sdc('docs/orchestration.md', '169')},
    ],
  },
  {
    id: 'slot',
    name: 'slot',
    repo: 'https://github.com/beyond10x/slot',
    site: 'https://beyond10x.github.io/slot/',
    is: 'One shared pool of build slots for every build on a machine.',
    isCite: readme('slot', '3'),
    use: 'used',
    supplies: [
      {text: 'The bound tools run a tree\'s task check and cargo test through the pool.', cite: sdc('driver/examples/sdc/tools/sdc-tool.sh', '185-201')},
      {text: 'Open: inside a Substrate sandbox slot would run the job locally and the pool would be lost (inferred, not run).', cite: sdc('docs/layers.md', '157')},
    ],
  },
];
