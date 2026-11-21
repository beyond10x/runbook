import type {ReactNode} from 'react';
import Link from '@docusaurus/Link';
import type {SiteIndex} from './types';
import {protocolHref} from './ui';

function Branch({index, id, current, seen}: {index: SiteIndex; id: string; current?: string; seen: Set<string>}): ReactNode {
  const p = index.protocols.find((x) => x.id === id);
  if (!p || seen.has(id)) return null;
  const next = new Set(seen).add(id);
  const kids = index.flow.filter((f) => f.parent === id).sort((a, b) => a.step - b.step);
  return (
    <li>
      <Link to={protocolHref(p.stem)} className={`sdc-rail__link${p.id === current ? ' is-current' : ''}`} aria-current={p.id === current ? 'page' : undefined}>
        <code>{p.id}</code>
        <span className="sdc-rail__meta">{p.actions} actions</span>
      </Link>
      {kids.length ? (
        <ul>
          {kids.map((k) => (
            <Branch key={k.child} index={index} id={k.child} current={current} seen={next} />
          ))}
        </ul>
      ) : null}
    </li>
  );
}

/** Every protocol, as the tree the case_open actions make: the root's children, then theirs. */
export default function ProtocolRail({index, current}: {index: SiteIndex; current?: string}): ReactNode {
  return (
    <nav className="sdc-rail" aria-label="Protocols">
      <p className="sdc-rail__title">Protocols</p>
      <ul>
        {index.roots.map((r) => (
          <Branch key={r} index={index} id={r} current={current} seen={new Set()} />
        ))}
      </ul>
    </nav>
  );
}
