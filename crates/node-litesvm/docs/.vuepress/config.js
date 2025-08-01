const path = require('path');

module.exports = {
    /**
     * Ref：https://v1.vuepress.vuejs.org/config/#title
     */
    title: 'LiteSVM',
    base: "/litesvm/",
    head: [
      ['link', { rel: 'icon', href: '/public/favicon.ico', type: "image/x-icon" }]
    ],
    /**
     * Theme configuration, here is the default theme configuration for VuePress.
     *
     * ref：https://v1.vuepress.vuejs.org/theme/default-theme-config.html
     */
    themeConfig: {
      repo: 'LiteSVM/litesvm',
      editLinks: false,
      docsDir: 'docs',
      editLinkText: '',
      lastUpdated: false,
      nav: [
        {
          text: 'Tutorial',
          link: '/tutorial/',
        },
        {
          text: 'API Reference',
          link: '/api/',
        },
      ],
      sidebar: {
        '/tutorial': 'auto',
        '/': 'auto',
      }
    },
  
    /**
     * Apply plugins，ref：https://v1.vuepress.vuejs.org/zh/plugin/
     */
    plugins: [
      [
        'vuepress-plugin-typedoc',
        {
          entryPoints: [
            path.resolve(__dirname, '../../litesvm/index.ts')
          ],
          tsconfig: path.resolve(__dirname, '../../tsconfig.json'),
          out: 'api',
          cleanOutputDir: true,
          debug: true,
        },
      ],
    ],
    extendMarkdown: (md) => {
        // use more markdown-it plugins!
        md.use(require('markdown-it-include'))
    }
  };
