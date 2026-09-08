# CretSpec

**Start with the spec.** CretSpec reads a project's spec repository, clones its code and prepares the exact development guidelines it selects.

```sh
cspec project clone CretQL-spec
```

## Result

```text
Projects/
  CretQL/                       code repository
  CretQL-spec/                  spec repository and project definition
    project.json
    guidelines.lock.json
    spec/
    .local/                     ignored, generated files
      guidelines/<commit>/
```

No project manifest, marker or editor file is created in the parent directory. The spec is the entry point for project commands.

## Install on Windows, Linux or macOS

Install Git and Node.js 22 or later, including npm. Git must have access to the repositories through SSH or HTTPS with a credential manager.

Install a fixed copy of the tool from its source:

```sh
git clone <CretSpec-repository-url> CretSpec
cd CretSpec
npm pack
npm install --global ./cretspec-0.2.0.tgz --ignore-scripts --no-audit --no-fund
cspec --version
```

There is no npm registry release or standalone installer yet. To update the installation, package and install the new version. Changes to the source directory do not change an installed tarball.

These commands work in PowerShell, Bash and Zsh. If PowerShell blocks a script launcher, use `npm.cmd` or `cspec.cmd`. The global npm prefix must be writable by your account.

Without a global installation, run `node bin/cspec.mjs` from the source directory. For development, `npm link` runs the changing source directly.

## Configure shared guidelines once

Clone your shared guidelines repository separately, then select it:

```sh
git clone <CretAI-repository-url> /path/to/CretAI
cspec guidelines set /path/to/CretAI
```

For example, Windows can use `cspec guidelines set "D:/GitHub/CretAI"`.

The personal setting is stored in `~/.cretspec/config.json`. It contains the editable guidelines location, not project definitions. Set CRETSPEC_HOME to use an isolated configuration directory.

A short spec name uses the same repository namespace as the configured guidelines clone's origin. If CretAI's origin is `git@github.com:owner/CretAI.git`, `CretQL-spec` resolves to `git@github.com:owner/CretQL-spec`. There is no separate per-project registry. Without an origin, short names resolve next to the local guidelines source.

## Clone a project

Run from a parent directory where the two new repositories should be created:

```sh
cspec project clone CretQL-spec
cd CretQL-spec
cspec project info
cspec project open
```

CretSpec first fetches the spec, reads project.json and guidelines.lock.json, and then fetches the code and pinned guidelines. The code directory is the sibling named by `project.json.name`. Spec names are exact repository names; the tool does not invent aliases or rename repositories.

Full URLs and explicit local paths also work:

```sh
cspec project clone git@github.com:owner/Example-spec.git
cspec project clone ./source/Example-spec ./projects/Example-spec
```

The optional destination is the **spec directory**, not an enclosing workspace. Use `./`, `../` or an absolute path for local sources. Relative code and guidelines URLs are resolved against the spec's source location, never its destination.

The spec must contain a valid manifest, lock and spec/ directory. See [the manifest format](docs/manifest.md). Templates come from the guidelines repository. Code and spec follow their default branches; only the guidelines are pinned.

Existing destinations are preserved. A failed clone can leave partial new directories for inspection; it does not delete sources or overwrite existing code.

## Read, open and edit

```sh
cspec project info
cspec project open --print
cspec guidelines edit
```

Run project commands inside the spec, or pass its directory. They read the current manifest and lock each time. Running inside the code repository requires an explicit spec path.

`project info` prints the resolved project context and prepares the selected guidelines snapshot. It does not persist a second definition.

`project open` generates `<spec>/.local/project.code-workspace` and opens it using the operating system's file association. `--print` generates the file and prints its path without launching an editor. VS Code supports this format; other editors can open the directories shown by `project info`.

Editor folders are regenerated from the spec. Other existing workspace settings are preserved. Deleting the generated editor file does not lose the project definition.

`guidelines edit` opens the shared editable guidelines clone. The active snapshot remains separate and fixed to the spec's lock.

## Existing clones

Place existing code and spec clones next to each other, using the manifest's project name for the code directory:

```sh
cspec project attach /projects/Example /projects/Example-spec
```

This prepares generated context inside the spec and preserves existing tracked files. Arbitrary path bindings are not stored elsewhere.

Version 0.2 replaces the enclosing workspace layout from 0.1. For an old trial, keep the original as a backup and clone into a new parent directory, or arrange its code and spec as siblings. The old outer marker and editor file are no longer used.

## Versions and limits

- Shared guidelines and the tool installation are versioned independently.
- Update a spec's manifest and lock together to adopt new guidance. Commands then prepare a snapshot under .local/guidelines/<commit>/.
- The selected commit must also exist in the editable guidelines clone. Run `git fetch --tags` there if necessary.
- Cached snapshots must match the lock and have no tracked changes or untracked files. They are detached checkouts, not filesystem-enforced read-only directories.
- Cloning does not install product dependencies, execute spec scripts or start an application.
- Skills and assistant integrations are not activated automatically.
- Spec Kit integration, spec generation, a standalone installer and automated releases remain future work.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md). CI checks syntax, integration scenarios, launchers and packaging on Windows, Linux and macOS with Node.js 22 and 24.
