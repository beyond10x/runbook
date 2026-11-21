import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {withProductSite} from '@beyond10x/docs-system/product-site';
import {PRISM_ADDITIONAL_LANGUAGES} from '@beyond10x/docs-system/code';

const siteDir = path.dirname(fileURLToPath(import.meta.url));

const config: Config = {
  title: 'Runbook',
  tagline: 'Agentic runbooks: fixed, checked ways to handle a case.',

  future: {v4: true},

  url: 'https://beyond10x.github.io',
  baseUrl: '/runbook/',

  organizationName: 'beyond10x',
  projectName: 'runbook',
  deploymentBranch: 'gh-pages',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  markdown: {
    format: 'detect',
    hooks: {onBrokenMarkdownLinks: 'throw'},
  },

  i18n: {defaultLocale: 'en', locales: ['en']},

  plugins: ['./plugins/sdc-data.ts'],

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl: 'https://github.com/beyond10x/runbook/tree/main/website/',
        },
        blog: false,
        theme: {customCss: './src/css/custom.css'},
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    prism: {additionalLanguages: [...PRISM_ADDITIONAL_LANGUAGES]},
    navbar: {
      title: 'Runbook',
      items: [
        {
          to: '/docs/',
          label: 'How it works',
          position: 'left',
          activeBaseRegex: '/docs/?$|/docs/(flow|context|built-on)/?$|/llm-is-not-the-runtime',
        },
        {to: '/protocols', label: 'Protocols', position: 'left', activeBaseRegex: '/protocols'},
        {to: '/evaluation', label: 'Evaluation', position: 'left', activeBaseRegex: '/evaluation'},
        {to: '/docs/run-it', label: 'Run it', position: 'left', activeBaseRegex: '/docs/run-it'},
        {href: 'https://github.com/beyond10x/runbook', position: 'right', className: 'sdc-navbar-github', 'aria-label': 'GitHub repository'},
      ],
    },
    footer: {
      links: [
        {
          title: 'Documentation',
          items: [
            {label: 'What Runbook is', to: '/docs/'},
            {label: 'The LLM is not the runtime', to: '/llm-is-not-the-runtime'},
            {label: 'The flow', to: '/docs/flow'},
            {label: 'Context', to: '/docs/context'},
            {label: 'Built on', to: '/docs/built-on'},
            {label: 'Protocols', to: '/protocols'},
            {label: 'Evaluation', to: '/evaluation'},
            {label: 'Run it', to: '/docs/run-it'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'Source', href: 'https://github.com/beyond10x/runbook'},
            {label: 'Changelog', href: 'https://github.com/beyond10x/runbook/blob/main/CHANGELOG.md'},
          ],
        },
      ],
      copyright: 'A beyond10x prototype. Runbook · Apache-2.0.',
    },
  } satisfies Preset.ThemeConfig,
};

const site = withProductSite(config, {mark: 'Rb'});

export default {
  ...site,
  staticDirectories: site.staticDirectories?.map((directory) =>
    directory && path.isAbsolute(directory) ? path.relative(siteDir, directory) : directory,
  ),
} satisfies Config;
