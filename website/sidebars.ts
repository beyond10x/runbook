import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const page = (id: string, label: string) => ({type: 'doc' as const, id, label});
const link = (href: string, label: string) => ({type: 'link' as const, href, label});

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Start',
      collapsed: false,
      items: [page('index', 'What it is'), link('/llm-is-not-the-runtime', 'The LLM is not the runtime'), page('run-it', 'Run it')],
    },
    {
      type: 'category',
      label: 'The cycle',
      collapsed: false,
      items: [page('flow', 'The flow'), page('context', 'Context'), link('/protocols', 'Protocol browser')],
    },
    {
      type: 'category',
      label: 'Context',
      collapsed: false,
      items: [page('built-on', 'What it is built on'), link('/evaluation', 'Evaluation')],
    },
  ],
};

export default sidebars;
