import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import path from 'node:path';
import type {LoadContext, Plugin, PluginConfig, RouteConfig} from '@docusaurus/types';
import {withProductSite} from '@beyond10x/docs-system/product-site';
import {PRISM_ADDITIONAL_LANGUAGES} from '@beyond10x/docs-system/code';
import {patchbackDark} from './src/prism/themes';

const config: Config = {
  title: 'Patchback',
  tagline: 'From a recording to an editable synth patch, its notes and a rendering to compare.',
  favicon: 'img/mark.svg',

  future: {
    v4: true,
  },

  url: 'https://beyond10x.github.io',
  baseUrl: '/patchback/',

  organizationName: 'beyond10x',
  projectName: 'patchback',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  markdown: {
    format: 'detect',
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },

  themes: ['@docusaurus/theme-mermaid'],

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl: 'https://github.com/beyond10x/patchback/tree/main/website/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    prism: {
      theme: patchbackDark,
      darkTheme: patchbackDark,
      additionalLanguages: [...PRISM_ADDITIONAL_LANGUAGES],
    },
    navbar: {
      title: 'Patchback',
      items: [
        {to: '/docs/', label: 'Docs', position: 'left', activeBasePath: '/docs'},
        {to: '/docs/concepts/closed-loop', label: 'Concepts', position: 'left', activeBasePath: '/docs/concepts'},
        {to: '/docs/guides/render-a-patch', label: 'Guides', position: 'left', activeBasePath: '/docs/guides'},
        {to: '/docs/reference/cli', label: 'Reference', position: 'left', activeBasePath: '/docs/reference'},
        {to: '/docs/status', label: 'Status', position: 'left'},
        {href: 'https://github.com/beyond10x/patchback', label: 'GitHub', position: 'right'},
      ],
    },
    footer: {
      links: [
        {
          title: 'Start here',
          items: [
            {label: 'Overview', to: '/docs/'},
            {label: 'Getting started', to: '/docs/getting-started'},
            {label: 'Status', to: '/docs/status'},
          ],
        },
        {
          title: 'Reference',
          items: [
            {label: 'CLI', to: '/docs/reference/cli'},
            {label: 'Files', to: '/docs/reference/files'},
            {label: 'Parameters', to: '/docs/reference/parameters'},
            {label: 'ESS specification', to: '/docs/reference/ess'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'Source', href: 'https://github.com/beyond10x/patchback'},
            {label: 'Design', href: 'https://github.com/beyond10x/patchback/blob/main/DESIGN.md'},
            {label: 'ModelSpec', href: 'https://beyond10x.github.io/modelspec/'},
            {label: 'ESS', href: 'https://beyond10x.github.io/ess/'},
          ],
        },
      ],
      copyright: 'A beyond10x project. Patchback · Apache-2.0.',
    },
  } satisfies Preset.ThemeConfig,
};

type PluginFactory = (context: LoadContext, options: unknown) => Plugin;

function portable(site: Config): Config {
  const relative = (directory: string | undefined) =>
    directory && path.isAbsolute(directory)
      ? path.relative(__dirname, directory).split(path.sep).join('/')
      : directory;
  const aliased = (factory: PluginFactory): PluginFactory =>
    function portableModules(context, options) {
      const plugin = factory(context, options);
      const contentLoaded = plugin.contentLoaded?.bind(plugin);
      if (!contentLoaded) return plugin;
      const alias = (module: unknown) =>
        typeof module === 'string' && module.startsWith(context.generatedFilesDir)
          ? '@generated/' + path.relative(context.generatedFilesDir, module).split(path.sep).join('/')
          : module;
      return {
        ...plugin,
        contentLoaded: ({content, actions}) =>
          contentLoaded({
            content,
            actions: {
              ...actions,
              addRoute: (route: RouteConfig) =>
                actions.addRoute({
                  ...route,
                  modules: Object.fromEntries(
                    Object.entries(route.modules ?? {}).map(([name, module]) => [name, alias(module)]),
                  ) as RouteConfig['modules'],
                }),
            },
          }),
      };
    };
  const plugins = (site.plugins ?? []) as unknown[];
  return {
    ...site,
    staticDirectories: (site.staticDirectories ?? ['static']).map(relative) as string[],
    plugins: plugins.map((entry) =>
      Array.isArray(entry) && typeof entry[0] === 'function'
        ? [aliased(entry[0] as PluginFactory), entry[1]]
        : entry,
    ) as PluginConfig[],
  };
}

export default portable(withProductSite(config, {landing: './product.json', mark: 'Pb'}));
