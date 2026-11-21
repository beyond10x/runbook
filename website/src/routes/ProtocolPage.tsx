import {useMemo, useState, type ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import {useHistory} from '@docusaurus/router';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useBrokenLinks from '@docusaurus/useBrokenLinks';
import ProtocolGraph, {KIND_LABEL} from '../components/ProtocolGraph';
import ProtocolRail from '../components/ProtocolRail';
import {ExecutorChip, Fact, InlineMd, PredicateList, executorLabel, protocolHref} from '../components/ui';
import type {Executor, ProtocolAction, ProtocolDoc, SiteIndex} from '../components/types';

const KINDS: Executor['kind'][] = ['program', 'model', 'person', 'child-case', 'copy'];

function ExecutorDetail({a, index}: {a: ProtocolAction; index: SiteIndex}): ReactNode {
  const e = a.executor;
  const stemOf = (id?: string) => index.protocols.find((p) => p.id === id)?.stem;
  const rows: [string, ReactNode][] = [];
  if (e.program) rows.push(['Program', <code>{e.program}</code>]);
  if (e.kind === 'model') {
    rows.push(['Node class', <code>{e.node_class}</code>]);
    if (e.default_model) rows.push(['Default model', <code>{e.default_model}</code>]);
    if (e.operations?.length) rows.push(['Operations', e.operations.map((o) => <code key={o}>{o}</code>)]);
  }
  if (e.kind === 'child-case' && e.protocol) {
    const stem = stemOf(e.protocol);
    rows.push(['Opens', <>{stem ? <Link to={protocolHref(stem)}><code>{e.protocol}</code></Link> : <code>{e.protocol}</code>}{e.each ? <> · one case per entry of <code>{e.each}</code></> : ' · one case'}</>]);
  }
  if (e.kind === 'copy') rows.push(['Copies', <><code>{e.record}</code> from <code>{e.from}</code></>]);
  if (e.kind === 'person' && !e.bound) rows.push(['Binding', e.note ?? 'none: a person takes this action outside the driver']);
  if (e.producer) rows.push(['Producer', <><code>{e.producer}</code>{e.independent ? ' · listed as independent' : ''}</>]);
  if (e.results) rows.push(['Exit code → result', Object.entries(e.results).map(([k, v]) => <code key={k}>{`${k} → ${v ?? 'none'}`}</code>)]);
  if (e.retry_when?.length) rows.push(['Runs again after', e.retry_when.map((r) => <code key={r}>{r}</code>)]);
  if (e.max_runs) rows.push(['At most', `${e.max_runs} runs per arrival`]);
  if (e.handoff) rows.push(['Hand-off', 'hands over to a person when it has done what it may']);
  return <>{rows.map(([k, v]) => <div className="sdc-kv" key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</>;
}

function ActionCard({a, doc, index, selected, onSelect}: {a: ProtocolAction; doc: ProtocolDoc; index: SiteIndex; selected: string | null; onSelect: (id: string | null) => void}): ReactNode {
  useBrokenLinks().collectAnchor(`action-${a.name}`);
  const id = `action:${a.name}`;
  const pressed = selected === id;
  return (
    <li className={`sdc-action${pressed ? ' is-selected' : ''}`} id={`action-${a.name}`}>
      <div className="sdc-action__head">
        <h3><code>{a.name}</code></h3>
        <ExecutorChip executor={a.executor} />
        <span className="sdc-tag">step {a.step + 1}</span>
        <span className="sdc-tag">{a.effect}</span>
        <button type="button" className="sdc-button" aria-pressed={pressed} onClick={() => onSelect(pressed ? null : id)}>
          {pressed ? 'Shown in graph' : 'Show in graph'}
        </button>
      </div>
      <p className="sdc-action__desc">{a.description}</p>
      <dl className="sdc-action__facts">
        <div className="sdc-kv">
          <dt>Requires</dt>
          <dd><PredicateList lines={a.precondition} onRef={(r) => onSelect(r)} /></dd>
        </div>
        <div className="sdc-kv">
          <dt>Records</dt>
          <dd>{a.produces.length ? a.produces.map((k) => <button key={k} type="button" className="sdc-ref" onClick={() => onSelect(`evidence:${k}`)}><code>{k}</code></button>) : 'nothing'}</dd>
        </div>
        {a.waits_for.length ? (
          <div className="sdc-kv"><dt>Waits for</dt><dd>{a.waits_for.map((w) => <a key={w} href={`#action-${w}`}><code>{w}</code></a>)}</dd></div>
        ) : null}
        {a.capabilities.length ? (
          <div className="sdc-kv"><dt>Needs authority</dt><dd>{a.capabilities.map((c) => <code key={c}>{c}</code>)}</dd></div>
        ) : null}
        {a.decision_class ? (
          <div className="sdc-kv"><dt>Decision class</dt><dd><code>{a.decision_class.class}</code> · {a.decision_class.type}{a.decision_class.side ? `, ${a.decision_class.side} side` : ''}</dd></div>
        ) : null}
        {a.arguments.length ? <div className="sdc-kv"><dt>Arguments</dt><dd>{a.arguments.map((x) => <code key={x}>{x}</code>)}</dd></div> : null}
        <ExecutorDetail a={a} index={index} />
      </dl>
      <p className="sdc-sr">Executor: {executorLabel(a.executor)}. Protocol {doc.id}.</p>
    </li>
  );
}

export default function ProtocolPage({protocol, index}: {protocol: ProtocolDoc; index: SiteIndex}): ReactNode {
  const doc = protocol;
  const [selected, setSelected] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [kinds, setKinds] = useState<Set<Executor['kind']>>(new Set());
  const history = useHistory();
  const base = useBaseUrl('/');
  const present = KINDS.filter((k) => doc.actions.some((a) => a.executor.kind === k));
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return doc.actions.filter((a) => {
      if (kinds.size && !kinds.has(a.executor.kind)) return false;
      if (!q) return true;
      const hay = [a.name, a.description, a.produces.join(' '), a.executor.program ?? '', a.executor.node_class ?? '', executorLabel(a.executor), a.capabilities.join(' ')].join(' ').toLowerCase();
      return hay.includes(q);
    });
  }, [doc, query, kinds]);
  const toggle = (k: Executor['kind']) => {
    const next = new Set(kinds);
    if (next.has(k)) next.delete(k);
    else next.add(k);
    setKinds(next);
  };
  const stemOf = (id: string) => index.protocols.find((p) => p.id === id)?.stem;
  const selectedNode = selected ? doc.graph.nodes.find((n) => n.id === selected) : undefined;
  return (
    <Layout title={`${doc.id} · Protocol browser`} description={doc.summary}>
      <div className="sdc-browser container">
        <ProtocolRail index={index} current={doc.id} />
        <main className="sdc-browser__main" id="protocol-main">
          <nav className="sdc-crumbs" aria-label="Breadcrumb">
            <Link to="/protocols">Protocols</Link>
            {doc.parents.map((p) => (
              <span key={p.protocol}> / <Link to={protocolHref(p.stem)}>{p.protocol}</Link></span>
            ))}
            <span aria-current="page"> / {doc.id}</span>
          </nav>
          <header className="sdc-head">
            <p className="sdc-kicker">Protocol · revision {doc.revision}</p>
            <h1><code>{doc.id}</code></h1>
            <p className="sdc-lede">{doc.summary}</p>
          </header>
          <dl className="sdc-facts" aria-label="Key facts">
            <Fact label="Actions" value={doc.actions.length} />
            <Fact label="Evidence kinds" value={doc.evidence.length} />
            <Fact label="Claims" value={doc.claims.length} />
            <Fact label="Outcomes" value={doc.outcomes.length} />
            <Fact label="States (canon check)" value={doc.check.states ?? 'not in the check output'} />
            <Fact label="Simulate findings" value={doc.simulate.findings ?? 'no pinned report'} />
          </dl>
          <p className="sdc-links">
            <a href={doc.source.url}>The YAML at tag {doc.source.tag ?? 'main'} on GitHub</a>
            {doc.bindings ? <> · bindings in <code>{doc.bindings}</code></> : <> · no bindings file</>}
          </p>
          {doc.parents.length ? (
            <p>Opened by {doc.parents.map((p, i) => <span key={p.protocol}>{i ? ', ' : ''}<Link to={protocolHref(p.stem)}><code>{p.protocol}</code></Link> through <code>{p.action}</code></span>)}.</p>
          ) : (
            <p>No protocol opens this one: a run starts it as a root case.</p>
          )}
          {doc.children.length ? (
            <div className="sdc-children">
              <h2 id="children">Child cases</h2>
              <ol>
                {doc.children.map((c) => (
                  <li key={c.action}>
                    {c.stem ? <Link to={protocolHref(c.stem)}><code>{c.protocol}</code></Link> : <code>{c.protocol}</code>}{' '}
                    opened by <a href={`#action-${c.action}`}><code>{c.action}</code></a>, {c.each ? <>one case per entry of <code>{c.each}</code></> : 'one case'}, about <code>{c.subject}</code>
                  </li>
                ))}
              </ol>
            </div>
          ) : null}
          <h2 id="description">What it does</h2>
          <p>{doc.description}</p>
          <ProtocolGraph doc={doc} selected={selected} onSelect={setSelected} onOpenChild={(id) => { const s = stemOf(id); if (s) history.push(`${base}protocols/${s}`); }} />
          <p className="sdc-live" aria-live="polite">
            {selectedNode ? <>Selected: {KIND_LABEL[selectedNode.kind].toLowerCase()} <code>{selectedNode.name}</code>. <button type="button" className="sdc-button" onClick={() => setSelected(null)}>Clear</button></> : 'Nothing selected.'}
          </p>

          <h2 id="actions">Actions</h2>
          <p className="sdc-muted">In the order they can first be taken: an action comes after the actions whose evidence its precondition needs true. Guards (a claim that must still be unknown) do not order.</p>
          <div className="sdc-filter">
            <label className="sdc-search">
              <span>Search actions</span>
              <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="name, record, program…" />
            </label>
            <div role="group" aria-label="Filter by executor" className="sdc-filter__chips">
              {present.map((k) => (
                <button key={k} type="button" className={`sdc-chip sdc-chip--${k} sdc-chip--toggle`} aria-pressed={kinds.has(k)} onClick={() => toggle(k)}>
                  {k === 'person' ? 'Person' : executorLabel({kind: k} as Executor)} ({doc.actions.filter((a) => a.executor.kind === k).length})
                </button>
              ))}
            </div>
          </div>
          <p className="sdc-muted" aria-live="polite">{shown.length} of {doc.actions.length} actions shown.</p>
          <ol className="sdc-actions">
            {shown.map((a) => <ActionCard key={a.name} a={a} doc={doc} index={index} selected={selected} onSelect={setSelected} />)}
          </ol>

          <h2 id="outcomes">Outcomes</h2>
          <ul className="sdc-outcomes">
            {doc.outcomes.map((o) => (
              <li key={o.name}>
                <h3><code>{o.name}</code>{o.decision_class ? <span className="sdc-tag">closed under decision class {o.decision_class.class}</span> : null}</h3>
                <p>{o.description}</p>
                <PredicateList lines={o.requires} onRef={(r) => setSelected(r)} />
                {doc.simulate.by_outcome?.[o.name] !== undefined ? <p className="sdc-muted">Terminable in {doc.simulate.by_outcome[o.name]} reachable states of the pinned simulate report.</p> : null}
              </li>
            ))}
          </ul>

          <h2 id="claims">Claims</h2>
          <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Claims">
            <table>
              <thead><tr><th scope="col">Claim</th><th scope="col">True when</th><th scope="col">Meaning</th></tr></thead>
              <tbody>
                {doc.claims.map((c) => (
                  <tr key={c.name}><th scope="row"><code>{c.name}</code></th><td><PredicateList lines={c.true_when} /></td><td>{c.description}</td></tr>
                ))}
              </tbody>
            </table>
          </div>

          <h2 id="evidence">Evidence kinds</h2>
          <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="Evidence kinds">
            <table>
              <thead><tr><th scope="col">Evidence</th><th scope="col">Recorded by</th><th scope="col">Meaning</th></tr></thead>
              <tbody>
                {doc.evidence.map((e) => (
                  <tr key={e.name}><th scope="row"><code>{e.name}</code></th><td>{e.produced_by.length ? e.produced_by.map((p) => <code key={p}>{p}</code>) : 'child cases or the runtime'}</td><td>{e.description}{e.max_age ? ` (applies for ${e.max_age})` : ''}</td></tr>
                ))}
              </tbody>
            </table>
          </div>

          {doc.obligations.length ? (
            <>
              <h2 id="obligations">Obligations</h2>
              <ul className="sdc-plain">
                {doc.obligations.map((o) => <li key={o.name}><code>{o.name}</code>: {o.description}</li>)}
              </ul>
            </>
          ) : null}

          <h2 id="checks">Checks</h2>
          <ul className="sdc-plain">
            <li>
              <code>canon check</code>: {doc.check.states !== null ? <>{doc.check.states.toLocaleString('en')} states at revision {doc.check.revision}</> : doc.check.note} (<code>{doc.check.source}</code>).
            </li>
            <li>
              <code>canon simulate</code>{doc.simulate.canon ? <> at Canon <code>{doc.simulate.canon}</code></> : null}:{' '}
              {doc.simulate.findings !== null ? (
                <>{doc.simulate.reachable} reachable states, {doc.simulate.terminable} terminable, {doc.simulate.findings} findings ({doc.simulate.stuck_or_cycle} stuck or cycle states)</>
              ) : doc.simulate.note}
              {doc.simulate.source ? <> (<code>{doc.simulate.source}</code>; triage in <code>docs/simulate-findings.md</code>)</> : null}.
            </li>
          </ul>
          {doc.artifacts.length ? (
            <>
              <h2 id="artifacts">Artifacts</h2>
              <ul className="sdc-plain">{doc.artifacts.map((a) => <li key={a.name}><code>{a.name}</code>: <InlineMd text={a.description} /></li>)}</ul>
            </>
          ) : null}
        </main>
      </div>
    </Layout>
  );
}
