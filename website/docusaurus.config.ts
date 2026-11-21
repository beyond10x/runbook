import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {withProductSite} from '@beyond10x/docs-system/product-site';
import {PRISM_ADDITIONAL_LANGUAGES} from '@beyond10x/docs-system/code';

const siteDir = path.dirname(fileURLToPath(import.meta.url));

const config: Config = {
  title: 'SDC',
  tagline: 'A software delivery cycle that runs on Canon protocols.',

  future: {v4: true},

  url: 'https://beyond10x.github.io',
  baseUrl: '/sdc/',

  organizationName: 'beyond10x',
  projectName: 'sdc',
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
          editUrl: 'https://github.com/beyond10x/sdc/tree/main/website/',
        },
        blog: false,
        theme: {customCss: './src/css/custom.css'},
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    prism: {additionalLanguages: [...PRISM_ADDITIONAL_LANGUAGES]},
    navbar: {
      title: 'SDC',
      items: [
        {to: '/docs/', label: 'What it is', position: 'left', activeBaseRegex: '^/sdc/docs/?$'},
        {to: '/llm-is-not-the-runtime', label: 'The LLM is not the runtime', position: 'left'},
        {to: '/docs/flow', label: 'The flow', position: 'left'},
        {to: '/protocols', label: 'Protocols', position: 'left', activeBaseRegex: '^/sdc/protocols'},
        {to: '/docs/built-on', label: 'Built on', position: 'left'},
        {to: '/evaluation', label: 'Evaluation', position: 'left', activeBaseRegex: '^/sdc/evaluation'},
        {to: '/docs/run-it', label: 'Run it', position: 'left'},
        {href: 'https://github.com/beyond10x/sdc', label: 'GitHub', position: 'right'},
      ],
    },
    footer: {
      links: [
        {
          title: 'Documentation',
          items: [
            {label: 'What it is', to: '/docs/'},
            {label: 'The LLM is not the runtime', to: '/llm-is-not-the-runtime'},
            {label: 'The flow', to: '/docs/flow'},
            {label: 'Protocol browser', to: '/protocols'},
            {label: 'What it is built on', to: '/docs/built-on'},
            {label: 'Evaluation', to: '/evaluation'},
            {label: 'Run it', to: '/docs/run-it'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'Source', href: 'https://github.com/beyond10x/sdc'},
            {label: 'Changelog', href: 'https://github.com/beyond10x/sdc/blob/main/CHANGELOG.md'},
          ],
        },
      ],
      copyright: 'A beyond10x prototype. SDC · Apache-2.0.',
    },
  } satisfies Preset.ThemeConfig,
};

const site = withProductSite(config, {mark: 'Sd'});

export default {
  ...site,
  staticDirectories: site.staticDirectories?.map((directory) =>
    directory && path.isAbsolute(directory) ? path.relative(siteDir, directory) : directory,
  ),
} satisfies Config;
