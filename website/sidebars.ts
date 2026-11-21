import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const page = (id: string, label: string) => ({type: 'doc' as const, id, label});
const link = (href: string, label: string) => ({type: 'link' as const, href, label});

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'How it works',
      collapsed: false,
      items: [
        page('index', 'What Runbook is'),
        link('/llm-is-not-the-runtime', 'The LLM is not the runtime'),
        page('flow', 'The flow'),
        page('context', 'Context'),
        page('built-on', 'Built on'),
      ],
    },
    page('run-it', 'Run it'),
  ],
};

export default sidebars;
