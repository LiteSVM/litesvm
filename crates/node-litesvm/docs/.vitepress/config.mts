import { defineConfig } from 'vitepress'
import typedocSidebar from '../api/typedoc-sidebar.json';

export default defineConfig({
  title: "LiteSVM",
  description: "A VitePress Site",
  base: "/litesvm",
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Tutorial', link: '/tutorial' },
      { text: 'API', link: '/api/' }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/LiteSVM/litesvm' }
    ]
  },
})
