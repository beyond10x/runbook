import type {ReactNode} from 'react';
import {COMPONENTS, KERNEL, LAYERS, USE_LABEL, type Component, type Use} from '../data/layers';

const GLYPH: Record<Use, string> = {used: '●', partly: '◐', 'not-yet': '○', planned: '◌'};

function byId(id: string): Component {
  return COMPONENTS.find((c) => c.id === id)!;
}

function Chip({c}: {c: Component}): ReactNode {
  return (
    <a href={`#${c.id}`} className={`sdc-layer__chip sdc-use--${c.use}`}>
      <span className="sdc-layer__name">{c.name}</span>
      <span className="sdc-layer__use"><span aria-hidden="true">{GLYPH[c.use]} </span>{USE_LABEL[c.use]}</span>
    </a>
  );
}

/** The layer stack of docs/layers.md § 3, top to bottom, with Canon beside it as the kernel. */
export function LayerDiagram(): ReactNode {
  const kernel = byId(KERNEL);
  return (
    <figure className="sdc-layers" aria-labelledby="sdc-layers-caption">
      <div className="sdc-layers__grid">
        <ol className="sdc-layers__stack" aria-label="Layers, top to bottom">
          {LAYERS.map((l) => (
            <li key={l.n} className={`sdc-layer${l.members.includes('sdc') ? ' is-sdc' : ''}`}>
              <p className="sdc-layer__label"><span className="sdc-layer__n">{l.n}</span> {l.name}</p>
              <div className="sdc-layer__members">
                {l.members.map((m) => <Chip key={m} c={byId(m)} />)}
                {l.note ? <span className="sdc-layer__note">{l.note}</span> : null}
              </div>
            </li>
          ))}
        </ol>
        <aside className="sdc-layers__kernel" aria-label="Kernel, beside the stack">
          <p className="sdc-layer__label">Kernel, not a layer</p>
          <Chip c={kernel} />
          <p className="sdc-layer__note">A pure library the runtime calls, with no IO (<code>docs/layers.md:103-104</code>).</p>
        </aside>
      </div>
      <figcaption id="sdc-layers-caption" className="sdc-muted">
        The stack as <code>docs/layers.md</code> § 3 draws it, applications on top. Each chip says whether Runbook
        uses the component today, in part, not yet, or as a plan; the glyph and the words carry the same, so nothing
        depends on colour.
      </figcaption>
    </figure>
  );
}

export function ComponentSections(): ReactNode {
  return (
    <>
      {COMPONENTS.filter((c) => c.id !== 'sdc').map((c) => (
        <section key={c.id} className="sdc-component" aria-labelledby={c.id}>
          <h2 id={c.id}>{c.name}</h2>
          <p className={`sdc-use sdc-use--${c.use}`}><span aria-hidden="true">{GLYPH[c.use]} </span>{USE_LABEL[c.use]}</p>
          <p>
            {c.is} <span className="sdc-cite">(<a href={c.isCite.href}>{c.isCite.label}</a>)</span>
          </p>
          <h3 className="sdc-component__h">What it supplies to Runbook</h3>
          <ul>
            {c.supplies.map((s) => (
              <li key={s.text}>{s.text} <span className="sdc-cite">(<a href={s.cite.href}>{s.cite.label}</a>)</span></li>
            ))}
          </ul>
          {c.temporary?.length ? (
            <>
              <h3 className="sdc-component__h">Temporary in Runbook</h3>
              <ul>
                {c.temporary.map((s) => (
                  <li key={s.text}>{s.text} <span className="sdc-cite">(<a href={s.cite.href}>{s.cite.label}</a>)</span></li>
                ))}
              </ul>
            </>
          ) : null}
          <p className="sdc-links">
            <a href={c.repo}>Repository: {c.repo.replace('https://github.com/', '')}</a>
            {c.site ? <> · <a href={c.site}>Documentation site</a></> : null}
          </p>
        </section>
      ))}
    </>
  );
}
