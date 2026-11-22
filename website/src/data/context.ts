/**
 * Facts for the context page (website/docs/context.mdx) that the generated protocol data does not
 * hold: the input list and the reply form of three model nodes, as the model-node script of the pinned commit builds them,
 * and the layers of one node's context. Every entry cites the line it was read from, on the commit
 * pinned in SRC.
 */

export const SRC = 'https://github.com/beyond10x/runbook/blob/7bbdee60c4595bf2beee8406a3f45b984def5dba';

export interface Cite {
  path: string;
  lines: string;
}

export function citeHref(c: Cite): string {
  const [a, b] = c.lines.split('-');
  return `${SRC}/${c.path}#L${a}${b ? `-L${b}` : ''}`;
}

export function citeLabel(c: Cite): string {
  return `${c.path.split('/').pop()}:${c.lines}`;
}

const NODE = 'driver/examples/sdc/tools/llm-node.sh';

export interface Input {
  file: string;
  what: string;
  from: 'earlier node' | 'you' | 'program' | 'protocol';
  when?: string;
}

export interface NodeFacts {
  action: string;
  stem: string;
  binding: Cite;
  inputs: Input[];
  inputsCite: Cite;
  returns: string;
  returnsCite: Cite;
}

/** Inputs every node of class writer or implement can get on top of its own list. */
export const SOMETIMES: Input[] = [
  {file: 'findings.yaml', what: 'open and decided findings: follow each decision', from: 'you', when: 'when the file exists'},
  {file: 'feedback.md', what: 'the checks that failed on its last output, with their logs', from: 'program', when: 'on a retry'},
  {file: 'rejection.json', what: 'the reasons an approval or review rejected its last output', from: 'you', when: 'on a rework take'},
];

export const NODES: NodeFacts[] = [
  {
    action: 'spec.author',
    stem: 'sdc-spec',
    binding: {path: 'driver/examples/sdc/sdc-spec.bindings.json', lines: '19-64'},
    inputs: [
      {file: 'idea.yaml', what: 'the idea text', from: 'you'},
      {file: 'requirements.yaml', what: 'the requirements list', from: 'earlier node'},
      {file: 'context.yaml', what: 'audience, platform, deployment and the other project defaults', from: 'earlier node'},
      {file: 'spec/', what: 'the current specification, if any (also copied to out/ to edit)', from: 'earlier node'},
      {file: 'example-spec/', what: 'a complete, valid example specification to copy the format from', from: 'program'},
    ],
    inputsCite: {path: NODE, lines: '346-369'},
    returns: 'Files: the specification tree out/spec/. Only that tree is copied back.',
    returnsCite: {path: NODE, lines: '713-715'},
  },
  {
    action: 'change.implement',
    stem: 'sdc-story',
    binding: {path: 'driver/examples/sdc/sdc-story.bindings.json', lines: '26-63'},
    inputs: [
      {file: 'requirements.yaml', what: 'the requirements list', from: 'earlier node'},
      {file: 'context.yaml', what: 'the project defaults', from: 'earlier node'},
      {file: 'spec/', what: 'the specification that passed its gate', from: 'earlier node'},
      {file: 'aep/story/<story>.md', what: 'the story to implement', from: 'earlier node'},
      {file: 'impl/', what: 'the code so far (also copied to out/ to extend)', from: 'earlier node'},
    ],
    inputsCite: {path: NODE, lines: '379-398'},
    returns: 'Files: the code tree out/impl/. Only that tree is copied back.',
    returnsCite: {path: NODE, lines: '713-715'},
  },
  {
    action: 'change.review',
    stem: 'sdc-story',
    binding: {path: 'driver/examples/sdc/sdc-story.bindings.json', lines: '82-109'},
    inputs: [
      {file: 'aep/story/<story>.md', what: 'the story', from: 'earlier node'},
      {file: 'scope.json', what: 'the commands, acceptance lines and requirements the story covers', from: 'program'},
      {file: 'commands.json', what: 'those commands in the compiled specification', from: 'program'},
      {file: 'impl/', what: 'the code tree change.implement wrote', from: 'earlier node'},
      {file: 'change.diff', what: 'what this story changed against the mainline', from: 'program'},
      {file: 'tests.txt', what: 'the test run that passed on this revision', from: 'program'},
      {file: 'gate.txt', what: 'the specification gate result', from: 'program'},
      {file: 'uncovered.json', what: 'requirements no story covers', from: 'program'},
    ],
    inputsCite: {path: NODE, lines: '399-428'},
    returns: 'A JSON reply under a response schema: decision, findings, summary. A program checks every cited line; an invalid reply is asked again at most twice.',
    returnsCite: {path: NODE, lines: '600-636'},
  },
];

export type Owner = 'protocol' | 'binding' | 'earlier node' | 'you';

export interface Layer {
  name: string;
  holds: string;
  owner: Owner;
  lever: string;
  cite: Cite;
}

/** One node's context, change.implement, from the outside in. */
export const LAYERS: Layer[] = [
  {
    name: 'When it runs',
    holds: 'Called only while the protocol admits it: the claim gate.passed holds, so the specification passed its gate.',
    owner: 'protocol',
    lever: 'the action’s precondition in protocol/sdc-story.protocol.yaml',
    cite: {path: 'protocol/sdc-story.protocol.yaml', lines: '160-167'},
  },
  {
    name: 'Session',
    holds: 'A new session. Operations: dir.list, file.read, file.write, file.edit, search. No shell, no web. At most 40 turns and 2 USD.',
    owner: 'binding',
    lever: 'llm.operations, llm.max_turns, llm.max_budget_usd in the bindings file',
    cite: {path: 'driver/examples/sdc/sdc-story.bindings.json', lines: '40-49'},
  },
  {
    name: 'Model and effort',
    holds: 'claude-haiku-5-5 from the binding, effort high from node-classes.json, unless a run setting names others.',
    owner: 'you',
    lever: 'SDC_LLM_MODEL_IMPLEMENT, SDC_LLM_EFFORT_IMPLEMENT',
    cite: {path: NODE, lines: '561-568'},
  },
  {
    name: 'Instruction',
    holds: 'A fixed preamble ("one step", "no shell", "programs check what you write") and the task text of this node.',
    owner: 'binding',
    lever: 'the node’s instruction files under instructions/, which its binding names',
    cite: {path: NODE, lines: '540-559'},
  },
  {
    name: 'Earlier products',
    holds: 'requirements.yaml, context.yaml, spec/, the story file, impl/: copies of the current revisions.',
    owner: 'earlier node',
    lever: 'a new revision: the earlier node runs again, or a person edits the file in work/',
    cite: {path: NODE, lines: '379-381'},
  },
  {
    name: 'Feedback',
    holds: 'feedback.md after a failed test run; rejection.json after a rejected review or merge.',
    owner: 'binding',
    lever: 'retry_when in the bindings file names which failed checks come back',
    cite: {path: 'driver/src/main.rs', lines: '1467-1507'},
  },
  {
    name: 'Decisions',
    holds: 'Interview answers and finding decisions, already applied to requirements.yaml; decided findings in findings.yaml.',
    owner: 'you',
    lever: 'answer the question, decide the finding, reject with reasons',
    cite: {path: NODE, lines: '470-491'},
  },
];

export const NOT_IN: Array<{name: string; why: string; cite: Cite}> = [
  {name: 'Goals document', why: 'reaches only the requester decider, no writer node', cite: {path: 'driver/examples/sdc/tools/decider.sh', lines: '179-192'}},
  {name: 'Earlier conversations', why: 'every call is a new session in a new directory', cite: {path: NODE, lines: '211-216'}},
];
