import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import specData from '@site/data/protocols/sdc-spec.json';
import storyData from '@site/data/protocols/sdc-story.json';
import {NODES, SOMETIMES, citeHref, citeLabel, type Cite, type Input, type NodeFacts} from '../data/context';
import type {ProtocolAction} from './types';
import {PredicateList, protocolHref} from './ui';
import './context.css';

const DATA: Record<string, {actions: ProtocolAction[]}> = {
  'sdc-spec': specData as unknown as {actions: ProtocolAction[]},
  'sdc-story': storyData as unknown as {actions: ProtocolAction[]},
};

/** The operations a model node gets when its binding names none (line 246 of the model-node script at the pinned commit). */
const DEFAULT_OPS = ['dir.list', 'file.edit', 'file.read', 'file.write', 'search'];

function action(n: NodeFacts): ProtocolAction | undefined {
  return DATA[n.stem]?.actions.find((a) => a.name === n.action);
}

function node(name: string): NodeFacts {
  return NODES.find((n) => n.action === name)!;
}

export function Src({c}: {c: Cite}): ReactNode {
  return (
    <a className="sdc-ctx-src" href={citeHref(c)}>
      <code>{citeLabel(c)}</code>
    </a>
  );
}

function Arrow({label}: {label: string}): ReactNode {
  return (
    <div className="sdc-ctx-arrow" aria-hidden="true">
      <svg viewBox="0 0 48 16" width="48" height="16" focusable="false">
        <path d="M2 8 H40" />
        <path d="M34 2 L44 8 L34 14" />
      </svg>
      <span>{label}</span>
    </div>
  );
}

function InputList({items, mark}: {items: Input[]; mark?: (i: Input) => string | null}): ReactNode {
  return (
    <ul className="sdc-ctx-stack">
      {items.map((i) => {
        const m = mark?.(i) ?? null;
        return (
          <li key={i.file} className={m ? 'is-marked' : undefined}>
            <code>{i.file}</code>
            {m ? <span className="sdc-ctx-tag sdc-ctx-tag--from">{m}</span> : null}
          </li>
        );
      })}
    </ul>
  );
}

function NotPassed({items}: {items: string[]}): ReactNode {
  return (
    <div className="sdc-ctx-not">
      <p className="sdc-ctx-label">Not passed on</p>
      <ul>
        {items.map((t) => (
          <li key={t}>
            <span aria-hidden="true">✕ </span>
            <s>{t}</s>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * Graphic 1: two nodes of one story case with the case store between them. Node A writes a
 * product; a program records evidence about it; node B gets its own input list, which holds A's
 * product and not A's session.
 */
export default function ContextFlow(): ReactNode {
  const a = node('change.implement');
  const b = node('change.review');
  const fromA = (i: Input) => (i.file === 'impl/' ? 'A’s product' : i.file === 'tests.txt' ? 'evidence' : null);
  return (
    <figure className="sdc-ctx-flow" data-testid="context-flow" aria-labelledby="sdc-ctx-flow-cap">
      <div className="sdc-ctx-flow__grid">
        <section className="sdc-ctx-card" aria-label="Node A, change.implement">
          <p className="sdc-ctx-kicker"><span className="sdc-ctx-n">1</span> Node A · model call</p>
          <h3 className="sdc-ctx-card__name"><code>{a.action}</code></h3>
          <p className="sdc-ctx-label">Gets, copied into <code>in/</code></p>
          <InputList items={a.inputs} />
          <p className="sdc-ctx-note">and the instruction text of this node</p>
          <p className="sdc-ctx-label">Hands back</p>
          <p className="sdc-ctx-out"><code>out/impl/</code></p>
          <NotPassed items={['its prompt and replies', 'its reasoning', 'its tool calls and their output', 'other files it wrote']} />
          <p className="sdc-ctx-note">These stay in A’s call record.</p>
        </section>
        <Arrow label="product" />
        <section className="sdc-ctx-store" aria-label="The case store">
          <p className="sdc-ctx-kicker"><span className="sdc-ctx-n">2</span> Case store · the program</p>
          <ol className="sdc-ctx-store__rows">
            <li><strong>Product.</strong> <code>work/impl/</code> gets a new revision.</li>
            <li><strong>Evidence.</strong> A program, <code>tests.run</code>, records <code>conformance_run = passed</code> about that revision.</li>
            <li><strong>Event log.</strong> Who ran, the exit code, the log.</li>
            <li><strong>Next.</strong> Canon finds <code>change.review</code> admissible.</li>
          </ol>
        </section>
        <Arrow label="copies" />
        <section className="sdc-ctx-card" aria-label="Node B, change.review">
          <p className="sdc-ctx-kicker"><span className="sdc-ctx-n">3</span> Node B · model call</p>
          <h3 className="sdc-ctx-card__name"><code>{b.action}</code></h3>
          <p className="sdc-ctx-label">Gets, copied into <code>in/</code></p>
          <InputList items={b.inputs} mark={fromA} />
          <p className="sdc-ctx-label">Hands back</p>
          <p className="sdc-ctx-out">a JSON reply: <code>approved</code> or <code>rejected</code> with findings</p>
          <NotPassed items={['A’s conversation', 'A’s reasoning', 'A’s tool output']} />
        </section>
      </div>
      <div className="sdc-ctx-flow__back">
        <span className="sdc-ctx-n">4</span>
        <p>
          On <code>rejected</code>, the reasons are recorded. A’s next take is a new session. It gets the same inputs
          plus <code>rejection.json</code> and must address each reason.
        </p>
      </div>
      <figcaption id="sdc-ctx-flow-cap" className="sdc-muted">
        Two model nodes of one story case, read 1 to 4. Inputs are the lists in <Src c={a.inputsCite} /> and{' '}
        <Src c={b.inputsCite} />. Struck-through items exist only in the earlier call’s record and are never copied to a
        later node.
      </figcaption>
    </figure>
  );
}

/** One model node: what the protocol data says about it, and the input list the model-node script builds. */
export function NodeCard({name}: {name: string}): ReactNode {
  const n = node(name);
  const a = action(n);
  const ex = a?.executor;
  const ops = ex?.operations ?? DEFAULT_OPS;
  const sometimes = ex?.node_class === 'review' ? [] : SOMETIMES;
  return (
    <section className="sdc-ctx-node" aria-labelledby={`node-${n.action}`}>
      <h3 id={`node-${n.action}`} className="sdc-ctx-node__h">
        <Link to={`${protocolHref(n.stem)}#action-${n.action}`}><code>{n.action}</code></Link>
        <span className="sdc-chip sdc-chip--model">class {ex?.node_class ?? 'unknown'}</span>
      </h3>
      <dl className="sdc-ctx-kv">
        <dt>Runs while</dt>
        <dd>{a ? <PredicateList lines={a.precondition} /> : 'not in the protocol data'}</dd>
        <dt>Session</dt>
        <dd>new for every call · default model <code>{ex?.default_model ?? 'unknown'}</code> · at most {ex?.max_runs ?? '?'} runs</dd>
        <dt>May use</dt>
        <dd>{ops.map((o) => <code key={o}>{o}</code>)}<span className="sdc-muted">no shell, no web</span></dd>
        <dt>Gets</dt>
        <dd>
          <ul className="sdc-ctx-inputs">
            {n.inputs.map((i) => (
              <li key={i.file}><code>in/{i.file}</code> <span>{i.what}</span> <span className="sdc-ctx-tag">{i.from}</span></li>
            ))}
            {sometimes.map((i) => (
              <li key={i.file} className="is-sometimes"><code>in/{i.file}</code> <span>{i.what}</span> <span className="sdc-ctx-tag">{i.when}</span></li>
            ))}
          </ul>
        </dd>
        <dt>Sent back by</dt>
        <dd>{ex?.retry_when?.length ? ex.retry_when.map((k) => <code key={k}>{k}</code>) : <span className="sdc-muted">nothing: no retry_when</span>}</dd>
        <dt>Hands back</dt>
        <dd>{n.returns}</dd>
        <dt>Source</dt>
        <dd><Src c={n.inputsCite} /> <Src c={n.binding} /> <Src c={n.returnsCite} /></dd>
      </dl>
    </section>
  );
}
