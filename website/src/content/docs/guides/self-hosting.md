---
title: Use your own Git hosting
description: Use CretSpec with private repositories, another Git server or local sources.
---

CretSpec runs locally; there is no CretSpec service to deploy. Host your three Git repositories wherever your team normally hosts code. Private repositories work through your existing Git authentication.

Supported sources include HTTPS, SSH URLs, SCP-style SSH references and local Git paths. For example:

```sh
cspec config namespace ssh://git@git.example.com/team
cspec project clone Atlas-spec
```

```sh
cspec project clone git@git.example.com:team/Atlas-spec.git
```

Use a full source for a repository on another server. Relative sources such as `../Atlas` and `../ai-guidelines` resolve beside the spec's source. Remote specs cannot point to absolute local repositories.

## Authentication

Test access first with `git ls-remote <repository-url>`. Git uses your configured SSH agent or credential helper. CretSpec does not maintain its own credentials. Do not embed tokens or passwords in manifests, URLs or personal settings.

CretSpec invokes system Git using argument arrays. It does not run scripts from your project's spec. Git's own configuration, credential helpers and filters remain your responsibility, as with an ordinary clone.

## Local and offline use

For a local trial, keep committed source repositories next to each other and pass an explicit path to the spec, such as `./sources/Atlas-spec`. Existing prepared projects can be inspected offline. Fetching unavailable remote commits naturally requires connectivity. Copying only the source files without their `.git` directories does not create a working project.

## Host these docs yourself

The documentation is a static Astro/Starlight site. In `website/`, run `npm ci` and `npm run build` with Node.js 24. Serve the generated `website/dist/` directory with any static web server. Set `site` and `base` in `website/astro.config.mjs` to match your host and URL path before building.

For GitHub Pages, choose **GitHub Actions** as the Pages source and set repository variable `PAGES_ENABLED` to `true`. The documentation workflow builds the site and deploys through the `github-pages` environment. A private repository's Pages availability depends on your GitHub plan and settings.
