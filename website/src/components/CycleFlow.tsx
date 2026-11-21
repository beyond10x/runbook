import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import {usePluginData} from '@docusaurus/useGlobalData';
import type {SiteIndex} from './types';
import {protocolHref} from './ui';

export function useSiteIndex(): SiteIndex {
  return (usePluginData('sdc-data') as {index: SiteIndex}).index;
}

const SUCCESS = new Set(['passed', 'operating', 'fixed']);

function Ends({outcomes}: {outcomes: string[]}): ReactNode {
  return (
    <p className="sdc-flow__ends">
      <span className="sdc-flow__label">Ends</span>
      {outcomes.map((o) => (
        <code key={o} className={SUCCESS.has(o) ? 'is-success' : undefined}>{SUCCESS.has(o) ? `✓ ${o}` : o}</code>
      ))}
    </p>
  );
}

type Via = {action: string; each: string | null};

function Box({index, id, via, summary}: {index: SiteIndex; id: string; via?: Via; summary?: boolean}): ReactNode {
  const p = index.protocols.find((x) => x.id === id);
  if (!p) return null;
  const kids = index.flow.filter((f) => f.parent === id).length;
  return (
    <div className={`sdc-flow__box${kids ? ' has-children' : ''}`}>
      {via ? (
        <p className="sdc-flow__via">
          opened by <code>{via.action}</code>
          {via.each ? <span className="sdc-flow__each"> · one case per entry of <code>{via.each}</code></span> : null}
        </p>
      ) : (
        <p className="sdc-flow__via">root case</p>
      )}
      <Link to={protocolHref(p.stem)} className="sdc-flow__name">
        <code>{p.id}</code>
      </Link>
      {summary ? <p className="sdc-flow__summary">{p.summary}</p> : null}
      <p className="sdc-flow__meta">{p.actions} actions{p.states !== null ? ` · ${p.states.toLocaleString('en')} states` : ''}{kids ? ` · opens ${kids} kinds of child case` : ''}</p>
      <Ends outcomes={p.outcomes} />
    </div>
  );
}

/** One parent's child cases as a row, then a row for every child that opens children of its own. */
function Rows({index, parent, seen}: {index: SiteIndex; parent: string; seen: Set<string>}): ReactNode {
  if (seen.has(parent)) return null;
  const next = new Set(seen).add(parent);
  const kids = index.flow.filter((f) => f.parent === parent).sort((a, b) => a.step - b.step);
  if (!kids.length) return null;
  return (
    <>
      <div className="sdc-flow__row">
        <p className="sdc-flow__rowlabel">
          <code>{parent}</code> opens, in order:
        </p>
        <ol className="sdc-flow__seq" aria-label={`Child cases of ${parent}, in the order they are opened`}>
          {kids.map((k, i) => (
            <li key={k.child} className="sdc-flow__step">
              {i > 0 ? <span className="sdc-flow__arrow" aria-hidden="true">→</span> : null}
              <Box index={index} id={k.child} via={{action: k.action, each: k.each}} />
            </li>
          ))}
        </ol>
      </div>
      {kids.map((k) => <Rows key={k.child} index={index} parent={k.child} seen={next} />)}
    </>
  );
}

/**
 * The main flow: the root protocol and the child cases each protocol opens, in the order their
 * case_open actions can be taken, drawn from the generated index alone.
 */
export default function CycleFlow({root = 'sdc'}: {root?: string}): ReactNode {
  const index = useSiteIndex();
  return (
    <figure className="sdc-flow" aria-labelledby="sdc-flow-title">
      <figcaption id="sdc-flow-title" className="sdc-flow__caption">
        The cycle as its protocols compose it. Each box is one protocol and links to it in the browser; an arrow
        means the next child case is opened only after the one before it ended (its <code>case_open</code> action
        waits for the earlier one). Ends marked ✓ are the success outcomes.
      </figcaption>
      <div className="sdc-flow__canvas">
        <div className="sdc-flow__root">
          <Box index={index} id={root} summary />
        </div>
        <Rows index={index} parent={root} seen={new Set()} />
      </div>
    </figure>
  );
}

/** One protocol's actions as steps, for a protocol that opens no child case (the bug fix). */
export function ActionFlow({id}: {id: string}): ReactNode {
  const index = useSiteIndex();
  const p = index.protocols.find((x) => x.id === id);
  if (!p) return null;
  const steps = new Map<number, typeof p.action_order>();
  for (const a of p.action_order) steps.set(a.step, [...(steps.get(a.step) ?? []), a]);
  const label: Record<string, string> = {program: 'program', model: 'model call', person: 'person', 'child-case': 'child case', copy: 'copy'};
  return (
    <figure className="sdc-flow sdc-flow--actions" aria-labelledby={`actions-${p.stem}`}>
      <figcaption id={`actions-${p.stem}`} className="sdc-flow__caption">
        <Link to={protocolHref(p.stem)}><code>{p.id}</code></Link>: its {p.actions} actions by step. An action is in a
        later step when its precondition needs evidence an earlier action records. Ends:{' '}
        {p.outcomes.map((o, i) => <span key={o}>{i ? ', ' : ''}<code>{o}</code></span>)}.
      </figcaption>
      <ol className="sdc-flow__seq sdc-flow__seq--actions">
        {[...steps.entries()].sort((a, b) => a[0] - b[0]).map(([s, acts], i) => (
          <li key={s} className="sdc-flow__step">
            {i > 0 ? <span className="sdc-flow__arrow" aria-hidden="true">→</span> : null}
            <div className="sdc-flow__box sdc-flow__box--step">
              <p className="sdc-flow__via">step {s + 1}</p>
              <ul>
                {acts.map((a) => (
                  <li key={a.name}>
                    <Link to={`${protocolHref(p.stem)}#action-${a.name}`}><code>{a.name}</code></Link>
                    <span className={`sdc-chip sdc-chip--${a.executor}`}>{label[a.executor] ?? a.executor}</span>
                  </li>
                ))}
              </ul>
            </div>
          </li>
        ))}
      </ol>
    </figure>
  );
}
