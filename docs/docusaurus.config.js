// @ts-check
// Note: type annotations allow type checking and IDEs autocompletion

const lightCodeTheme = require('prism-react-renderer/themes/github');
const darkCodeTheme = require('prism-react-renderer/themes/dracula');

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Grebuloff',
  tagline: 'Grebuloff is an experimental addon framework for Final Fantasy XIV.',

  // Set the production url of your site here
  url: 'https://grebuloff.ava.dev',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: 'avafloww', // Usually your GitHub org/user name.
  projectName: 'Grebuloff', // Usually your repo name.

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  // Even if you don't use internalization, you can use this field to set useful
  // metadata like html lang. For example, if your site is Chinese, you may want
  // to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          routeBasePath: '/',
          sidebarPath: require.resolve('./sidebars.js'),
          // Please change this to your repo.
          // Remove this to remove the "edit this page" links.
          editUrl:
            'https://github.com/avafloww/Grebuloff/tree/main/docs/',
        },
        blog: false,
        theme: {
          customCss: require.resolve('./custom.css'),
        },
      }),
    ],
  ],

  markdown: {
    mermaid: true,
  },

  stylesheets: [
    {
      href: 'https://use.fontawesome.com/releases/v5.15.4/css/all.css',
      integrity: 'sha384-DyZ88mC6Up2uqS4h/KRgHuoeGwBcD4Ng9SiP4dIRy0EXTlnuz47vAwmeGwVChigm',
      crossorigin: 'anonymous',
    }
  ],
  
  themes: ['@docusaurus/theme-mermaid'],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      // Replace with your project's social card
      image: 'img/grebuloff-social-card.jpg',
      navbar: {
        title: 'Grebuloff',
        logo: {
          alt: 'observe him',
          src: 'img/grebuloff-icon.jpg',
        },
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'main',
            position: 'left',
            label: 'Documentation',
          },
          {
            href: 'https://github.com/avafloww/Grebuloff',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      prism: {
        theme: lightCodeTheme,
        darkTheme: darkCodeTheme,
      }
    }),
};

module.exports = config;
