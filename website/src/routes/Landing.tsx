import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import {Feature, FeatureGrid, ProductHero, ProductSection} from '@beyond10x/docs-system/product';
import ProgressChart from '../components/ProgressChart';
import {InlineMd} from '../components/ui';
import type {SiteIndex} from '../components/types';

interface Props {
  index: SiteIndex;
  progressSvg: {svg: string};
  ledger: {pass_rule: string; release: string | null};
}

export default function Landing({index, progressSvg}: Props): ReactNode {
  const release = index.releases[0];
  const stages = index.protocols.filter((p) => p.id !== 'sdc.bugfix').length;
  return (
    <Layout title="SDC: a software delivery cycle on Canon protocols" description="A prototype software delivery cycle that runs on Canon protocols: the stage is derived from facts, and a model is only one action's executor." wrapperClassName="b10x-product-page">
      <main id="b10x-product-main">
        <div className="b10x-product">
        <ProductHero
          eyebrow="Prototype · software delivery cycle on Canon"
          promise={['From an idea to a delivered app,', 'one admissible action at a time.']}
          lede={
            <>
              The SDC is a prototype of a software delivery cycle that runs on Canon protocols: an idea goes through{' '}
              {stages} protocols (idea, intake, direction, epic, spec, plan, story, release), each a <code>protocol/1</code>{' '}
              case with child cases. Where a case stands is derived from facts, the files in its repository and the recorded
              check results, never from a stored phase. A driver runs the tool bound to one admissible action at a time, and a
              language model is only ever one action's executor: its output is an artifact revision, never evidence and
              never the next state.
            </>
          }
          actions={[
            {label: 'See the flow', href: '/docs/flow', kind: 'primary'},
            {label: 'Browse the protocols', href: '/protocols'},
            {label: 'Evaluation', href: '/evaluation'},
          ]}
          meta={release ? <>Release {release.version} · {release.date} · source release, no stable interface · Apache-2.0</> : 'No release yet'}
          aside={
            release ? (
              <div className="sdc-release-card">
                <p className="b10x-kicker">Current release</p>
                <p className="sdc-release-card__version">{release.version}</p>
                <p className="sdc-release-card__date">{release.date}</p>
                <p><InlineMd text={release.summary} /></p>
                {release.notes_url ? <p><a href={release.notes_url}>Release notes for {release.version}</a> (<code>{release.notes}</code>)</p> : null}
              </div>
            ) : undefined
          }
        />
        <ProductSection id="progress" number={1} eyebrow="Progress" title="How often an idea runs to a delivered app with no person." lede={<>Clean passes of ended runs per release and model setting. The figures and their limits are on the <Link to="/evaluation">evaluation</Link> page.</>}>
          <ProgressChart svg={progressSvg.svg} />
        </ProductSection>
        <ProductSection id="read" number={2} eyebrow="Read on" title="What the pages cover.">
          <FeatureGrid label="Pages">
            <Feature title="What it is" description="The definition, a run, autopilot and the decider, a clean pass." href="/docs/" />
            <Feature title="The LLM is not the runtime" description="Canon decides what is admissible; a model executes one action." href="/llm-is-not-the-runtime" />
            <Feature title="The flow" description="The main flow and its child cases, drawn from the protocols." href="/docs/flow" />
            <Feature title="Protocol browser" description={`All ${index.protocols.length} protocols, rendered from their YAML.`} href="/protocols" />
            <Feature title="What it is built on" description="Canon, Loom, Substrate, ESS, AEP and Gates: what each supplies today." href="/docs/built-on" />
            <Feature title="Run it" description="The scripted checks and a dry run with stub executors." href="/docs/run-it" />
          </FeatureGrid>
        </ProductSection>
        </div>
      </main>
    </Layout>
  );
}
