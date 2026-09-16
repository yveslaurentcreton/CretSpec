---
title: Configuration
description: Personal namespace settings and environment variables.
---

`cspec config namespace <value>` stores the namespace used by short names such as `cspec project clone Atlas-spec`.

```sh
cspec config namespace your-account
cspec config namespace https://github.com/your-account
cspec config namespace git@git.example.com:team
```

A bare GitHub owner expands to its HTTPS namespace. Explicit paths can select local sources. Run `cspec config namespace` without a value to see the current setting.

## File location

| Environment | Default personal file |
| --- | --- |
| Windows | `%USERPROFILE%\.cretspec\config.json` |
| macOS / Linux | `$HOME/.cretspec/config.json` |
| `CRETSPEC_HOME` set | `<CRETSPEC_HOME>/config.json` |

`CRETSPEC_HOME` is useful for isolated tests or separate personal configurations. Use an absolute path to keep it independent of your current directory.

```json
{
  "schemaVersion": 2,
  "repositoryNamespace": "https://github.com/your-account"
}
```

The file contains no registry, credentials or project-specific guideline version. Setting a namespace preserves unrelated existing preferences.

## Legacy settings

The earlier schema-version-1 personal configuration stored `guidelinesRoot`. CretSpec can derive a namespace from that clone's origin while it still exists. If it moved, set a namespace explicitly. The next namespace write replaces the legacy binding while preserving other preferences.

Full source URLs and already-cloned projects do not need a namespace setting. Help and version work without configuration. Authentication follows Git's environment and configuration.
