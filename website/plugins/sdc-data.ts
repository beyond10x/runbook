/**
 * Reads the committed data the site renders and adds the data routes.
 *
 * Inputs, all committed and all written by programs: `website/data/protocols/*.json`
 * (`sdc-protocol-site write`), `docs/progress.json` and `docs/img/progress.svg`
 * (`sdc-experiment progress`), `experiments/*.yaml`, `experiments/releases.yaml`, `experiments/targets.yaml` and
 * `experiments/<id>/timelines/*.json` (`sdc.run-timeline/1`), the committed run summaries
 * `experiments/<id>/runs/*.json`, `experiments/<id>/conformance/*.json` and
 * `experiments/<id>/sizes.json`, `experiments/<id>/scores.json` with its rubric, and `driver/examples/sdc/tools/node-classes.json`. Nothing
 * under the repository's `runs/` is read.
 * `SDC_SITE_FIXTURES=<dir>` replaces the repository's progress ledger, experiments and timelines
 * with the files in that directory, for the fixture build the tests use.
 */
import fs from 'node:fs/promises';
import path from 'node:path';
import yaml from 'js-yaml';
import type {LoadContext, Plugin} from '@docusaurus/types';

export interface SdcContent {
  index: any;
  protocols: Record<string, any>;
  progress: any;
  progressSvg: string;
  experiments: any[];
  releasesTable: any;
  targets: any;
  nodeClasses: any;
  timelines: any[];
  runSummaries: any[];
  conformance: any[];
  sizes: any[];
  quality: any[];
  fixtures: boolean;
}

async function readJson(file: string): Promise<any> {
  return JSON.parse(await fs.readFile(file, 'utf8'));
}

async function exists(file: string): Promise<boolean> {
  try {
    await fs.access(file);
    return true;
  } catch {
    return false;
  }
}

async function list(dir: string): Promise<string[]> {
  try {
    return (await fs.readdir(dir)).sort();
  } catch {
    return [];
  }
}

/** The experiment fields the evaluation page shows: inputs, settings, rules. No run output. */
function experimentSummary(doc: any): any {
  return {
    id: doc.id,
    kind: doc.kind,
    status: doc.status,
    title: doc.title ?? null,
    purpose: doc.purpose ?? null,
    created: doc.created ?? null,
    inputs: (doc.inputs ?? []).map((i: any) => ({name: i.name, kind: i.kind, text: typeof i.text === 'string' ? i.text : null})),
    tree_commit: typeof doc.tree_commit === 'string' ? doc.tree_commit : null,
    fixed: doc.fixed ?? {},
    variants: (doc.variants ?? []).map((v: any) => ({
      id: v.id,
      settings: v.settings ?? {},
      runs: (v.runs ?? []).map((r: any) => (typeof r === 'string' ? r : r.run)),
    })),
    runs_per_variant: doc.runs_per_variant ?? null,
    pass_rule: doc.pass_rule ?? null,
    decision_rule: doc.decision_rule ?? null,
    hypothesis: doc.hypothesis ?? null,
    expectation: doc.expectation ?? null,
    variable: doc.variable ?? null,
    baseline: Array.isArray(doc.baseline) ? doc.baseline : doc.baseline ? [doc.baseline] : [],
    treatment: doc.treatment ?? null,
    known_confounds: (doc.known_confounds ?? []).map((c: any) => ({key: c.key, note: c.note})),
    input_sha256: (doc.inputs ?? []).map((i: any) => i.sha256 ?? null),
    primary: doc.primary ?? null,
    verdict: doc.result?.verdict ?? null,
    why: doc.result?.why ?? null,
    comparisons: (doc.result?.comparisons ?? []).map((c: any) => ({
      metric: c.metric,
      baseline: c.baseline,
      treatment: c.treatment,
      direction: c.direction,
      verdict: c.verdict,
      why: c.why,
      p_one_sided: c.p_one_sided ?? null,
      baseline_values: c.baseline_values ? {median: c.baseline_values.median, min: c.baseline_values.min, max: c.baseline_values.max, n: c.baseline_values.n} : null,
      treatment_values: c.treatment_values ? {median: c.treatment_values.median, min: c.treatment_values.min, max: c.treatment_values.max, n: c.treatment_values.n} : null,
    })),
    variant_primaries: (doc.result?.variants ?? []).map((v: any) => ({
      id: v.id,
      primaries: Object.fromEntries(Object.entries(v.primaries ?? {}).map(([k, s]: [string, any]) => [k, {median: s.median, min: s.min, max: s.max, n: s.n}])),
    })),
  };
}

/**
 * The cost of one run per node class (`node-classes.json`): each node's own model cost goes to the
 * class of its action, the decider's cost on every node goes to `decider`. Null when the summary
 * has no node costs.
 */
function costByClass(nodes: any[], classes: Record<string, string[]>): Record<string, number> | null {
  const withCost = nodes.filter((n) => typeof n.cost_usd === 'number');
  if (!withCost.length) return null;
  const classOf = new Map<string, string>();
  for (const [cls, actions] of Object.entries(classes)) for (const a of actions) classOf.set(a, cls);
  const out: Record<string, number> = {};
  const add = (cls: string, v: number) => {
    if (v > 0) out[cls] = (out[cls] ?? 0) + v;
  };
  for (const n of withCost) {
    const decider = typeof n.decider_cost_usd === 'number' ? n.decider_cost_usd : 0;
    const action = typeof n.node === 'string' ? n.node.slice(n.node.indexOf('/') + 1) : '';
    add('decider', decider);
    add(classOf.get(action) ?? 'unclassified', n.cost_usd - decider);
  }
  return out;
}

/** The fields of a committed run summary the comparison shows (`sdc.run-metrics/1`, `sdc.free-agent-metrics/1`). */
function runSummary(doc: any, classes: Record<string, string[]>): any {
  const m = doc.metrics ?? {};
  const edges: any[] = Array.isArray(doc.edges) ? doc.edges : [];
  const nodes: any[] = Array.isArray(doc.nodes) ? doc.nodes : [];
  const sentBack = (from: string) => edges.filter((e) => e.from === from).reduce((a, e) => a + (e.count ?? 0), 0);
  const review = nodes.filter((n) => typeof n.node === 'string' && n.node.endsWith('/change.review'));
  return {
    run: doc.run,
    format: doc.format,
    cost_usd: m.cost_usd ?? null,
    cost_by_class: doc.format === 'sdc.run-metrics/1' ? costByClass(nodes, classes) : null,
    wall_s: m.wall_s ?? null,
    api_requests: m.api_requests ?? null,
    decider_calls: m.decider_calls ?? null,
    rework_takes: m.rework_takes ?? null,
    rework_cost_usd: m.rework_cost_usd ?? null,
    reviews: Array.isArray(doc.nodes) ? review.reduce((a, n) => a + (n.real_takes ?? 0), 0) : null,
    review_rejections: Array.isArray(doc.edges) ? sentBack('change.review:rejected') : null,
    merge_rejections: Array.isArray(doc.edges) ? sentBack('merge.approve:rejected') : null,
  };
}

/**
 * A quality evaluation (`sdc.quality-scores/1`): per product the score, the failed items, cost,
 * wall time and the exploratory findings; per side the score and passes per item; the rubric's
 * item names from its `- <id> <name>:` lines. No per-step evidence.
 */
function qualitySummary(experiment: string, doc: any, rubric: string): any {
  const items: Array<{id: string; name: string}> = [];
  for (const m of rubric.matchAll(/^- ([A-Z][0-9]+) ([^:\n]+):/gm)) items.push({id: m[1], name: m[2].trim()});
  return {
    experiment,
    n: doc.n ?? null,
    rubric: typeof doc.rubric === 'string' ? `experiments/${experiment}/${doc.rubric}` : null,
    items,
    sides: Object.entries(doc.sides ?? {}).map(([id, s]: [string, any]) => ({
      id,
      score: s.score ?? null,
      per_item_passes: s.per_item_passes ?? {},
      runs: (s.runs ?? []).map((r: string) => r.split('=')[0]),
    })),
    products: (doc.products ?? []).map((p: any) => ({
      run: p.run,
      side: p.side,
      score: p.score ?? null,
      n: p.n ?? null,
      not_testable: p.not_testable ?? null,
      failed: p.failed ?? [],
      cost_usd: p.cost_usd ?? null,
      wall_s: p.wall_s ?? null,
      findings: (p.exploratory_findings ?? []).map((f: any) => (typeof f === 'string' ? f : f.finding)),
    })),
  };
}

/** The targets (`experiments/targets.yaml`, `sdc.targets/1`): what each target is set on and its value. */
function targetsSummary(doc: any): any {
  if (doc?.format !== 'sdc.targets/1') return null;
  return {
    targets: (doc.targets ?? []).map((t: any) => ({
      id: t.id,
      metric: t.metric ?? null,
      input: t.input ?? null,
      target: typeof t.target === 'number' ? t.target : null,
      set: t.set ?? null,
      source_experiment: t.source_experiment ?? null,
    })),
  };
}

/** A conformance summary (`sdc.conformance-summary/1`) without its per-table detail. */
function conformanceSummary(doc: any): any {
  return {
    run: doc.run,
    measured: doc.measured ?? false,
    n: doc.n ?? null,
    k: doc.k ?? null,
    f: doc.f ?? null,
    p: doc.p ?? null,
    u: doc.u ?? null,
    tables_counted: doc.tables_counted ?? null,
    tables_written: doc.tables_written ?? null,
    summary: doc.summary ?? null,
  };
}

export default function sdcData(context: LoadContext): Plugin<SdcContent> {
  const site = context.siteDir;
  const repo = path.resolve(site, '..');
  const fixtureDir = process.env.SDC_SITE_FIXTURES ? path.resolve(site, process.env.SDC_SITE_FIXTURES) : undefined;
  const ledgerRoot = fixtureDir ?? repo;
  const protocolsDir = path.join(site, 'data', 'protocols');
  return {
    name: 'sdc-data',
    getPathsToWatch() {
      return [path.join(protocolsDir, '*.json'), path.join(ledgerRoot, 'docs', 'progress.json'), path.join(ledgerRoot, 'experiments', '**', '*')];
    },
    async loadContent() {
      const index = await readJson(path.join(protocolsDir, 'index.json'));
      const protocols: Record<string, any> = {};
      for (const p of index.protocols) protocols[p.stem] = await readJson(path.join(protocolsDir, `${p.stem}.json`));
      const progress = await readJson(path.join(ledgerRoot, 'docs', 'progress.json'));
      const progressSvg = await fs.readFile(path.join(repo, 'docs', 'img', 'progress.svg'), 'utf8');
      const expDir = path.join(ledgerRoot, 'experiments');
      const experiments: any[] = [];
      const timelines: any[] = [];
      const runSummaries: any[] = [];
      const conformance: any[] = [];
      const sizes: any[] = [];
      const quality: any[] = [];
      let releasesTable: any = null;
      let targets: any = null;
      const nodeClasses = await readJson(path.join(repo, 'driver', 'examples', 'sdc', 'tools', 'node-classes.json'));
      for (const name of await list(expDir)) {
        const file = path.join(expDir, name);
        if (name === 'releases.yaml') {
          releasesTable = yaml.load(await fs.readFile(file, 'utf8'));
        } else if (name === 'targets.yaml') {
          targets = targetsSummary(yaml.load(await fs.readFile(file, 'utf8')));
        } else if (name.endsWith('.yaml')) {
          experiments.push(experimentSummary(yaml.load(await fs.readFile(file, 'utf8'))));
        } else {
          for (const t of await list(path.join(file, 'timelines'))) {
            if (!t.endsWith('.json')) continue;
            const doc = await readJson(path.join(file, 'timelines', t));
            if (doc?.format === 'sdc.run-timeline/1') timelines.push({experiment: name, file: `experiments/${name}/timelines/${t}`, doc});
          }
          for (const r of await list(path.join(file, 'runs'))) {
            if (!r.endsWith('.json')) continue;
            const doc = await readJson(path.join(file, 'runs', r));
            if (doc?.format === 'sdc.run-metrics/1' || doc?.format === 'sdc.free-agent-metrics/1') runSummaries.push({experiment: name, ...runSummary(doc, nodeClasses.classes ?? {})});
          }
          for (const c of await list(path.join(file, 'conformance'))) {
            if (!c.endsWith('.json')) continue;
            const doc = await readJson(path.join(file, 'conformance', c));
            if (doc?.format === 'sdc.conformance-summary/1') conformance.push({experiment: name, ...conformanceSummary(doc)});
          }
          if (await exists(path.join(file, 'scores.json'))) {
            const doc = await readJson(path.join(file, 'scores.json'));
            if (doc?.format === 'sdc.quality-scores/1') {
              const rubricFile = path.join(file, typeof doc.rubric === 'string' ? doc.rubric : 'rubric.md');
              const rubric = (await exists(rubricFile)) ? await fs.readFile(rubricFile, 'utf8') : '';
              quality.push(qualitySummary(name, doc, rubric));
            }
          }
          if (await exists(path.join(file, 'sizes.json'))) {
            const doc = await readJson(path.join(file, 'sizes.json'));
            if (doc?.format === 'sdc.product-sizes/1') for (const p of doc.products ?? []) sizes.push({experiment: name, ...p});
          }
        }
      }
      return {index, protocols, progress, progressSvg, experiments, releasesTable, targets, nodeClasses, timelines, runSummaries, conformance, sizes, quality, fixtures: Boolean(fixtureDir)};
    },
    async contentLoaded({content, actions}) {
      const {createData, addRoute, setGlobalData} = actions;
      setGlobalData({index: content.index});
      const data = async (name: string, value: unknown) => createData(name, JSON.stringify(value));
      const indexData = await data('sdc-index.json', content.index);
      const route = (p: string) => path.posix.join(context.baseUrl, p);
      const latest = content.index.release?.version ?? null;
      addRoute({
        path: route('/'),
        exact: true,
        component: '@site/src/routes/Landing.tsx',
        modules: {
          index: indexData,
          progressSvg: await data('sdc-progress-svg.json', {svg: content.progressSvg}),
          ledger: await data('sdc-ledger-summary.json', {
            pass_rule: content.progress.pass_rule,
            latest: content.progress.entries.filter((e: any) => e.kind === 'release').slice(-1)[0] ?? null,
            release: latest,
          }),
        },
      });
      addRoute({path: route('/protocols'), exact: true, component: '@site/src/routes/ProtocolIndex.tsx', modules: {index: indexData}});
      for (const [stem, doc] of Object.entries(content.protocols)) {
        addRoute({
          path: route(`/protocols/${stem}`),
          exact: true,
          component: '@site/src/routes/ProtocolPage.tsx',
          modules: {index: indexData, protocol: await data(`sdc-protocol-${stem}.json`, doc)},
        });
      }
      const evaluation = await data('sdc-evaluation.json', {
        progress: content.progress,
        experiments: content.experiments,
        releasesTable: content.releasesTable,
        nodeClasses: content.nodeClasses,
        quality: content.quality,
        targets: content.targets,
        runs: content.runSummaries.map((r) => ({experiment: r.experiment, run: r.run, format: r.format, cost_usd: r.cost_usd, wall_s: r.wall_s, cost_by_class: r.cost_by_class})),
        fixtures: content.fixtures,
      });
      addRoute({path: route('/evaluation'), exact: true, component: '@site/src/routes/Evaluation.tsx', modules: {evaluation, index: indexData}});
      addRoute({
        path: route('/llm-is-not-the-runtime'),
        exact: true,
        component: '@site/src/routes/LlmIsNotTheRuntime.tsx',
        modules: {timelines: await data('sdc-timelines.json', {timelines: content.timelines, experiments: content.experiments, runs: content.runSummaries, conformance: content.conformance, sizes: content.sizes, quality: content.quality, fixtures: content.fixtures}), index: indexData},
      });
    },
  };
}
