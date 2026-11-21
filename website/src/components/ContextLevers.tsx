import type {ReactNode} from 'react';
import {LAYERS, NOT_IN, type Owner} from '../data/context';
import {Src} from './ContextFlow';
import './context.css';

const GLYPH: Record<Owner, string> = {protocol: '▣', binding: '▤', 'earlier node': '◆', you: '●'};

function OwnerChip({owner}: {owner: Owner}): ReactNode {
  return (
    <span className={`sdc-ctx-owner sdc-ctx-owner--${owner.replace(' ', '-')}`}>
      <span aria-hidden="true">{GLYPH[owner]} </span>
      {owner}
    </span>
  );
}

/**
 * Graphic 2: the context of one node, change.implement, as a stack of layers. Each layer names
 * who sets it and the lever that changes it.
 */
export default function ContextLevers(): ReactNode {
  return (
    <figure className="sdc-ctx-levers" data-testid="context-levers" aria-labelledby="sdc-ctx-levers-cap">
      <p className="sdc-ctx-kicker">The context of <code>change.implement</code>, outside in</p>
      <ol className="sdc-ctx-layers">
        {LAYERS.map((l, i) => (
          <li key={l.name} className="sdc-ctx-layer" style={{['--sdc-ctx-i' as string]: i}}>
            <div className="sdc-ctx-layer__head">
              <span className="sdc-ctx-layer__name">{l.name}</span>
              <OwnerChip owner={l.owner} />
            </div>
            <p className="sdc-ctx-layer__holds">{l.holds}</p>
            <p className="sdc-ctx-layer__lever">
              <span className="sdc-ctx-label">Lever</span> {l.lever} <Src c={l.cite} />
            </p>
          </li>
        ))}
      </ol>
      <div className="sdc-ctx-not sdc-ctx-not--wide">
        <p className="sdc-ctx-label">Not in this context</p>
        <ul>
          {NOT_IN.map((n) => (
            <li key={n.name}>
              <span aria-hidden="true">✕ </span>
              <s>{n.name}</s>: {n.why} <Src c={n.cite} />
            </li>
          ))}
        </ul>
      </div>
      <figcaption id="sdc-ctx-levers-cap" className="sdc-muted">
        Each layer says who sets it: the protocol, the binding (with the tool it names), an earlier node, or you. Glyph
        and word carry the same, so nothing depends on colour.
      </figcaption>
    </figure>
  );
}
