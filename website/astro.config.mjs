import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://yveslaurentcreton.github.io',
  base: '/CretSpec',
  trailingSlash: 'always',
  integrations: [starlight({
    title: 'CretSpec',
    description: 'One specification. A complete development project.',
    logo: {
      light: './src/assets/mark-light.svg',
      dark: './src/assets/mark-dark.svg',
    },
    favicon: '/favicon.svg',
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/yveslaurentcreton/CretSpec' }],
    customCss: ['./src/styles/custom.css'],
    expressiveCode: {
      themes: ['github-dark', 'github-light'],
      useStarlightUiThemeColors: true,
      styleOverrides: { borderRadius: '0.5rem' },
    },
    sidebar: [
      { label: 'Start here', items: [
        { label: 'Install', slug: 'start/install' },
        { label: 'Clone your first project', slug: 'start/quickstart' },
        { label: 'How it works', slug: 'start/concepts' },
      ] },
      { label: 'Everyday work', items: [
        { label: 'Work with your coding agent', slug: 'guides/agents' },
        { label: 'Start a specification', slug: 'guides/initialize' },
        { label: 'Work with guidelines', slug: 'guides/guidelines' },
        { label: 'Move or attach a project', slug: 'guides/existing-projects' },
        { label: 'Use your own Git hosting', slug: 'guides/self-hosting' },
        { label: 'Troubleshooting', slug: 'guides/troubleshooting' },
      ] },
      { label: 'Reference', items: [
        { label: 'Commands', slug: 'reference/commands' },
        { label: 'Project and lock files', slug: 'reference/manifest' },
        { label: 'Skills and agent context', slug: 'reference/skills' },
        { label: 'Configuration', slug: 'reference/configuration' },
      ] },
      { label: 'Contribute', items: [
        { label: 'Development', slug: 'contribute/development' },
        { label: 'For maintainers', slug: 'contribute/releases' },
      ] },
    ],
  })],
});
