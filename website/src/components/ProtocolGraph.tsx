import {useMemo, type KeyboardEvent, type ReactNode} from 'react';
import {layeredLayout} from '@beyond10x/docs-system/graph-layout';
import type {GraphEdge, GraphNode, ProtocolDoc} from './types';

export const KIND_LABEL: Record<GraphNode['kind'], string> = {
  action: 'Action',
  evidence: 'Evidence',
  child: 'Child case',
  claim: 'Claim',
  decision: 'Decision',
  outcome: 'Outcome',
};

const LAYER: Record<GraphNode['kind'], number> = {action: 0, evidence: 1, child: 1, claim: 2, decision: 2, outcome: 3};
const COLUMNS = ['Actions', 'Evidence and child cases', 'Claims and decisions', 'Outcomes'];
const CHAR = 7.9;
const HEIGHT = 46;

function nodeWidth(n: GraphNode): number {
  return Math.max(150, Math.ceil(n.name.length * CHAR) + 34);
}

/** What lights up when a node is selected: for an action, what it waits on and what it produces. */
export function litSet(doc: ProtocolDoc, selected: string | null): Set<string> | null {
  if (!selected) return null;
  const lit = new Set<string>([selected]);
  const edges = doc.graph.edges;
  const into = (id: string) => edges.filter((e) => e.to === id);
  const out = (id: string) => edges.filter((e) => e.from === id);
  for (const e of [...into(selected), ...out(selected)]) {
    lit.add(e.from);
    lit.add(e.to);
  }
  if (selected.startsWith('action:')) {
    for (const g of into(selected)) for (const e of into(g.from)) lit.add(e.from);
    for (const p of out(selected)) for (const e of out(p.to)) if (e.kind === 'establishes' || e.kind === 'reports') lit.add(e.to);
  }
  return lit;
}

function edgeLit(e: GraphEdge, lit: Set<string> | null, selected: string | null): boolean {
  if (!lit || !selected) return false;
  const otherAction = (id: string) => id.startsWith('action:') && id !== selected;
  return lit.has(e.from) && lit.has(e.to) && !otherAction(e.from) && !otherAction(e.to);
}

export interface ProtocolGraphProps {
  doc: ProtocolDoc;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onOpenChild?: (protocol: string) => void;
}

export default function ProtocolGraph({doc, selected, onSelect, onOpenChild}: ProtocolGraphProps): ReactNode {
  const layout = useMemo(() => {
    const nodes = doc.graph.nodes.map((n) => ({id: n.id, layer: LAYER[n.kind], width: nodeWidth(n), height: HEIGHT}));
    const edges = doc.graph.edges.map((e) => ({from: e.from, to: e.to, constraint: e.kind !== 'gates' && LAYER[kindOf(e.from)] < LAYER[kindOf(e.to)]}));
    return layeredLayout(nodes, edges, {layerGap: 90, nodeGap: 12, dummyGap: 8});
  }, [doc]);
  const lit = litSet(doc, selected);
  const byId = new Map(doc.graph.nodes.map((n) => [n.id, n]));
  const pad = {top: 44, left: 8, right: 8, bottom: 8};
  const width = layout.width + pad.left + pad.right;
  const height = layout.height + pad.top + pad.bottom;
  const descId = `graph-desc-${doc.stem}`;
  const counts = COLUMNS.map((_, i) => doc.graph.nodes.filter((n) => LAYER[n.kind] === i).length);
  const key = (event: KeyboardEvent, id: string) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onSelect(selected === id ? null : id);
    } else if (event.key === 'Escape') {
      onSelect(null);
    }
  };
  return (
    <figure className="b10x-graph sdc-graph" aria-labelledby={`graph-title-${doc.stem}`}>
      <figcaption className="b10x-graph__header">
        <div className="b10x-graph__heading">
          <p className="b10x-graph__kicker">Protocol graph · drawn from <code>{doc.source.path}</code></p>
          <strong className="b10x-graph__title" id={`graph-title-${doc.stem}`}>{doc.id}</strong>
          <p className="b10x-graph__description" id={descId}>
            Left to right: each action, the evidence it may record and the child case it opens, the claims that
            evidence makes true, and the outcomes those claims end the case with. A dashed line is a precondition: the
            claim must hold (or be unknown or false, as labelled) before the action is admissible. Select a node to light
            what it needs and what it gives; Escape clears.
          </p>
        </div>
        <ul className="b10x-graph__legend" aria-label="Node kinds">
          {(['action', 'evidence', 'child', 'claim', 'decision', 'outcome'] as const).map((k) => (
            <li key={k}><span className={`b10x-graph__swatch sdc-swatch--${k}`} aria-hidden="true" />{KIND_LABEL[k]}<span className="b10x-graph__count">{doc.graph.nodes.filter((n) => n.kind === k).length}</span></li>
          ))}
        </ul>
      </figcaption>
      <div className="b10x-graph__viewport" tabIndex={0} role="region" aria-label={`Graph of ${doc.id}; scroll sideways for the later columns`}>
        <div className="b10x-graph__canvas" style={{width}}>
          <svg className="b10x-graph__svg" viewBox={`0 0 ${width} ${height}`} width={width} height={height} role="group" aria-describedby={descId}>
            <defs>
              <marker id={`arrow-${doc.stem}`} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                <path d="M0,0 L8,4 L0,8 z" className="b10x-graph__arrow" />
              </marker>
              <marker id={`arrow-lit-${doc.stem}`} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                <path d="M0,0 L8,4 L0,8 z" className="b10x-graph__arrow--lit" />
              </marker>
            </defs>
            {layout.columns.map((c) => (
              <g key={c.index} aria-hidden="true">
                <text x={pad.left + c.x} y={16} className="b10x-graph__column-label">{COLUMNS[c.index]?.toUpperCase()} <tspan className="b10x-graph__column-index">{counts[c.index]}</tspan></text>
              </g>
            ))}
            <g transform={`translate(${pad.left} ${pad.top})`}>
              {layout.edges.map((le) => {
                const e = doc.graph.edges[le.index];
                const on = edgeLit(e, lit, selected);
                const cls = ['b10x-graph__edge', `b10x-graph__edge--${e.kind}`, on ? 'is-lit' : lit ? 'is-dimmed' : ''].join(' ');
                return <path key={le.index} d={le.path} className={cls} markerEnd={e.kind === 'gates' ? undefined : `url(#${on ? 'arrow-lit' : 'arrow'}-${doc.stem})`} />;
              })}
              {layout.edges.map((le) => {
                const e = doc.graph.edges[le.index];
                if (!e.qualifier || !edgeLit(e, lit, selected)) return null;
                const w = e.qualifier.length * 6.6 + 10;
                return (
                  <g key={`q${le.index}`} aria-hidden="true">
                    <rect x={le.label.x - w / 2} y={le.label.y - 9} width={w} height={17} rx={4} className="b10x-graph__qualifier-bg" />
                    <text x={le.label.x} y={le.label.y + 4} textAnchor="middle" className="b10x-graph__qualifier">{e.qualifier}</text>
                  </g>
                );
              })}
              {layout.nodes.map((ln) => {
                const n = byId.get(ln.id)!;
                const dim = lit && !lit.has(n.id);
                const pressed = selected === n.id;
                return (
                  <g
                    key={n.id}
                    className={`b10x-graph__node sdc-node--${n.kind}${dim ? ' is-dimmed' : ''}${lit?.has(n.id) && !pressed ? ' is-current' : ''}`}
                    transform={`translate(${ln.x} ${ln.y})`}
                    tabIndex={0}
                    role="button"
                    aria-pressed={pressed}
                    aria-label={`${KIND_LABEL[n.kind]} ${n.name}`}
                    data-node={n.id}
                    onClick={() => onSelect(pressed ? null : n.id)}
                    onDoubleClick={() => n.kind === 'child' && onOpenChild?.(n.name)}
                    onKeyDown={(ev) => key(ev, n.id)}
                  >
                    <rect className="b10x-graph__body" width={ln.width} height={ln.height} rx={7} />
                    <rect className="b10x-graph__accent" width={4} height={ln.height - 12} x={0} y={6} rx={2} />
                    <text>
                      <tspan x={14} y={18} className="b10x-graph__node-kicker">{KIND_LABEL[n.kind].toUpperCase()}</tspan>{' '}
                      <tspan x={14} y={35} className="b10x-graph__node-name">{n.name}</tspan>
                    </text>
                  </g>
                );
              })}
            </g>
          </svg>
        </div>
      </div>
    </figure>
  );
}

function kindOf(id: string): GraphNode['kind'] {
  return id.slice(0, id.indexOf(':')) as GraphNode['kind'];
}
