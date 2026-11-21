/**
 * Reads the committed data the site renders and adds the data routes.
 *
 * Inputs, all committed and all written by programs: `website/data/protocols/*.json`
 * (`sdc-protocol-site write`), `docs/progress.json` and `docs/img/progress.svg`
 * (`sdc-experiment progress`), `experiments/*.yaml`, `experiments/releases.yaml` and
 * `experiments/<id>/timelines/*.json` (`sdc.run-timeline/1`). Nothing under `runs/` is read.
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
  timelines: any[];
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
    primary: doc.primary ?? null,
    verdict: doc.result?.verdict ?? null,
    why: doc.result?.why ?? null,
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
      let releasesTable: any = null;
      for (const name of await list(expDir)) {
        const file = path.join(expDir, name);
        if (name === 'releases.yaml') {
          releasesTable = yaml.load(await fs.readFile(file, 'utf8'));
        } else if (name.endsWith('.yaml')) {
          experiments.push(experimentSummary(yaml.load(await fs.readFile(file, 'utf8'))));
        } else if (await exists(path.join(file, 'timelines'))) {
          for (const t of await list(path.join(file, 'timelines'))) {
            if (!t.endsWith('.json')) continue;
            const doc = await readJson(path.join(file, 'timelines', t));
            if (doc?.format === 'sdc.run-timeline/1') timelines.push({experiment: name, file: `experiments/${name}/timelines/${t}`, doc});
          }
        }
      }
      return {index, protocols, progress, progressSvg, experiments, releasesTable, timelines, fixtures: Boolean(fixtureDir)};
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
        fixtures: content.fixtures,
      });
      addRoute({path: route('/evaluation'), exact: true, component: '@site/src/routes/Evaluation.tsx', modules: {evaluation, index: indexData}});
      addRoute({
        path: route('/llm-is-not-the-runtime'),
        exact: true,
        component: '@site/src/routes/LlmIsNotTheRuntime.tsx',
        modules: {timelines: await data('sdc-timelines.json', {timelines: content.timelines, fixtures: content.fixtures}), index: indexData},
      });
    },
  };
}
