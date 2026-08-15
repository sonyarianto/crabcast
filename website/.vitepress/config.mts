import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Crabcast',
  description:
    'AzuraCast-style web radio management — multi-station, playlist automation, live DJ, requests, analytics. Rust + Crabsoup + Next.js.',
  lang: 'en-US',
  cleanUrls: true,
  // website/README.md is repo-facing (dev + Vercel deploy steps), not a
  // site page.
  srcExclude: ['README.md'],

  head: [
    ['meta', { name: 'theme-color', content: '#7c3aed' }],
  ],

  themeConfig: {
    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'API', link: '/guide/api' },
      { text: 'Architecture', link: '/architecture' },
      { text: 'GitHub', link: 'https://github.com/sonyarianto/crabcast' },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting started',
          items: [{ text: 'Install & run', link: '/guide/getting-started' }],
        },
        {
          text: 'Radio operation',
          items: [
            { text: 'Stations, playlists & live DJs', link: '/guide/stations' },
            { text: 'Requests & jingles', link: '/guide/requests-jingles' },
          ],
        },
        {
          text: 'Monitoring',
          items: [{ text: 'Analytics & alerts', link: '/guide/analytics' }],
        },
        {
          text: 'Reference',
          items: [
            { text: 'REST API & tokens', link: '/guide/api' },
            { text: 'Scaling & multi-host', link: '/guide/scaling' },
          ],
        },
      ],
    },

    footer: {
      message: 'MIT licensed',
      copyright: 'Copyright © 2026 Crabcast contributors',
    },

    search: {
      provider: 'local',
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/sonyarianto/crabcast' },
    ],
  },
})
