import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import ProtocolRail from '../components/ProtocolRail';
import CycleFlow, {ActionFlow} from '../components/CycleFlow';
import {protocolHref} from '../components/ui';
import type {SiteIndex} from '../components/types';

export default function ProtocolIndex({index}: {index: SiteIndex}): ReactNode {
  const others = index.roots.filter((r) => r !== 'sdc');
  return (
    <Layout title="Protocol browser" description="Every SDC protocol, rendered from its protocol/1 YAML.">
      <div className="sdc-browser container">
        <ProtocolRail index={index} />
        <main className="sdc-browser__main" id="protocol-main">
          <header className="sdc-head">
            <p className="sdc-kicker">Protocol browser</p>
            <h1>The {index.protocols.length} protocols</h1>
            <p className="sdc-lede">
              Each page is rendered from one <code>protocol/*.protocol.yaml</code> by <code>sdc-protocol-site</code>: its
              actions, what each requires and records, who or what runs it, its outcomes and the child cases it opens.
            </p>
          </header>
          <CycleFlow />
          {others.map((r) => <ActionFlow key={r} id={r} />)}
          <h2 id="all">All protocols</h2>
          <div className="sdc-table-wrap" tabIndex={0} role="region" aria-label="All protocols">
            <table>
              <thead>
                <tr><th scope="col">Protocol</th><th scope="col">Rev.</th><th scope="col">Actions</th><th scope="col">Outcomes</th><th scope="col">States</th><th scope="col">Simulate findings</th></tr>
              </thead>
              <tbody>
                {index.protocols.map((p) => (
                  <tr key={p.id}>
                    <th scope="row"><Link to={protocolHref(p.stem)}><code>{p.id}</code></Link></th>
                    <td>{p.revision}</td>
                    <td>{p.actions}</td>
                    <td>{p.outcomes.join(', ')}</td>
                    <td>{p.states !== null ? p.states.toLocaleString('en') : 'not in the check output'}</td>
                    <td>{p.simulate_findings ?? 'no report'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="sdc-muted">States: <code>docs/check-output.txt</code>, the committed <code>canon check</code> output. Findings: <code>protocol/simulate/*.txt</code>, the pinned <code>canon simulate</code> reports, triaged in <code>docs/simulate-findings.md</code>.</p>
        </main>
      </div>
    </Layout>
  );
}
