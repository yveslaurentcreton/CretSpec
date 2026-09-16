import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://yveslaurentcreton.github.io',
  base: '/CretSpec',
  trailingSlash: 'always',
  integrations: [starlight({
    title: 'CretSpec',
    description: 'One specification. A complete development project.',
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/yveslaurentcreton/CretSpec' }],
    customCss: ['./src/styles/custom.css'],
    sidebar: [
      { label: 'Start here', items: [
        { label: 'Install', slug: 'start/install' },
        { label: 'Clone your first project', slug: 'start/quickstart' },
        { label: 'How it works', slug: 'start/concepts' },
      ] },
      { label: 'Everyday work', items: [
        { label: 'Start a specification', slug: 'guides/initialize' },
        { label: 'Work with guidelines', slug: 'guides/guidelines' },
        { label: 'Move or attach a project', slug: 'guides/existing-projects' },
        { label: 'Use your own Git hosting', slug: 'guides/self-hosting' },
        { label: 'Troubleshooting', slug: 'guides/troubleshooting' },
      ] },
      { label: 'Reference', items: [
        { label: 'Commands', slug: 'reference/commands' },
        { label: 'Project and lock files', slug: 'reference/manifest' },
        { label: 'Configuration', slug: 'reference/configuration' },
      ] },
      { label: 'Contribute', items: [
        { label: 'Development', slug: 'contribute/development' },
        { label: 'Releases and packages', slug: 'contribute/releases' },
      ] },
    ],
  })],
});
