/** The shapes `sdc-protocol-site write` emits (`sdc.protocol-site/1`). */

export interface PredLine {
  depth: number;
  text: string;
  reference?: string;
  suffix?: string;
}

export interface Executor {
  kind: 'program' | 'model' | 'person' | 'child-case' | 'copy';
  bound?: boolean;
  note?: string;
  program?: string;
  node_class?: string;
  default_model?: string;
  operations?: string[];
  producer?: string;
  independent?: boolean;
  results?: Record<string, string | null>;
  retry_when?: string[];
  max_runs?: number;
  timeout_s?: number;
  handoff?: boolean;
  protocol?: string;
  each?: string;
  from?: string;
  record?: string;
}

export interface DecisionClass {
  class: string;
  type: string;
  side?: string;
}

export interface ProtocolAction {
  name: string;
  description: string;
  effect: string;
  step: number;
  capabilities: string[];
  produces: string[];
  waits_for: string[];
  arguments: string[];
  precondition: PredLine[];
  opens?: {protocol: string; subject: string};
  executor: Executor;
  decision_class?: DecisionClass;
}

export interface GraphNode {
  id: string;
  kind: 'action' | 'evidence' | 'child' | 'claim' | 'decision' | 'outcome';
  name: string;
}

export interface GraphEdge {
  from: string;
  to: string;
  kind: 'produces' | 'opens' | 'establishes' | 'reports' | 'supports' | 'requires' | 'decides' | 'gates';
  qualifier?: string;
}

export interface ProtocolDoc {
  id: string;
  stem: string;
  revision: number;
  description: string;
  summary: string;
  source: {path: string; url: string; tag: string | null};
  bindings: string | null;
  artifacts: {name: string; description: string}[];
  evidence: {name: string; description: string; produced_by: string[]; max_age?: string}[];
  claims: {name: string; description: string; true_when: PredLine[]}[];
  obligations: {name: string; description: string; discharged_when: PredLine[]}[];
  actions: ProtocolAction[];
  outcomes: {name: string; description: string; requires: PredLine[]; decision_class?: DecisionClass}[];
  invalidation: {name: string; description: string}[];
  children: {protocol: string; stem: string | null; action: string; subject: string; each: string | null; step: number}[];
  parents: {protocol: string; stem: string; action: string}[];
  check: {states: number | null; revision?: number; source: string; note?: string};
  simulate: {reachable?: number; terminable?: number; findings: number | null; stuck_or_cycle?: number; by_outcome?: Record<string, number>; canon?: string | null; source?: string; note?: string};
  graph: {nodes: GraphNode[]; edges: GraphEdge[]};
}

export interface ProtocolSummary {
  id: string;
  stem: string;
  revision: number;
  summary: string;
  actions: number;
  evidence: number;
  claims: number;
  outcomes: string[];
  children: string[];
  parents: string[];
  states: number | null;
  simulate_findings: number | null;
  action_order: {name: string; step: number}[];
}

export interface ReleaseInfo {
  version: string;
  date: string;
  summary: string;
  limits: string[];
  notes: string | null;
  notes_url: string | null;
}

export interface SiteIndex {
  repository: string;
  release: {version: string; date: string; tag: string; notes: string | null} | null;
  releases: ReleaseInfo[];
  protocols: ProtocolSummary[];
  roots: string[];
  flow: {parent: string; child: string; action: string; each: string | null; step: number}[];
}
