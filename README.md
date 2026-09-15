# CretSpec

**Start with the spec.** CretSpec creates a project folder containing its specification, code and editable development guidelines.

```sh
cspec project clone CretQL-spec
```

## Result

```text
Projects/
  CretQL/                        ordinary project root
    CretQL-spec/                  private spec repository
      project.json                the project definition
      guidelines.lock.json        exact guidelines version
      spec/
      .local/                     ignored, generated files
        guidelines/<commit>/
        project.code-workspace    generated when opening
    CretQL/                       independent code repository
    CretAI/                       editable guidelines repository
```

CretSpec is installed separately. Its source belongs outside the projects directory, for example in `D:/cspec-tools/CretSpec`. The project root contains no Git repository, manifest or editor file of its own.

## Install on Windows, Linux or macOS

Install Git and Node.js 22 or later, including npm. Git needs access to the repositories through SSH or HTTPS with your credential manager.

From a tools directory, install a fixed copy of CretSpec:

```sh
git clone <CretSpec-repository-url> CretSpec
cd CretSpec
npm pack
npm install --global ./cretspec-0.3.0.tgz --ignore-scripts --no-audit --no-fund
cspec --version
```

There is no npm registry release or standalone installer yet. Package and install each new version to update. Editing the source does not change an installed tarball.

These commands work in PowerShell, Bash and Zsh. If PowerShell blocks a script launcher, use `npm.cmd` or `cspec.cmd`. The global npm prefix must be writable by your account.

For development, use `node bin/cspec.mjs` or `npm link`.

## Configure the repository namespace once

```sh
cspec config namespace your-github-owner
```

This expands to `git@github.com:your-github-owner`. An HTTPS namespace also works:

```sh
cspec config namespace https://github.com/your-github-owner
```

The setting only tells CretSpec where to look for a short repository name. It is stored in `~/.cretspec/config.json`; set CRETSPEC_HOME for an isolated configuration directory. It contains no project definitions, project registry or editable guidelines path. Full spec URLs and explicit local paths need no namespace setting.

## Clone and open a project

Run from your projects directory:

```sh
cspec project clone CretQL-spec
cd CretQL
cspec project info
cspec project open
```

CretSpec fetches the spec first, reads its manifest and lock, and creates the project root named by `project.json.name`. Inside it, the spec keeps its repository name, the code directory uses the project name, and the editable guidelines directory uses its repository name.

Code, spec and editable guidelines follow their source's default branch. A separate guidelines snapshot is checked out at the exact locked commit inside the spec's ignored .local/ directory.

Full URLs and explicit local paths also work:

```sh
cspec project clone git@github.com:owner/Example-spec.git
cspec project clone ./source/Example-spec ./projects/MyExample
```

The optional destination is the **project root**. In the second example it contains Example-spec/, Example/ and CretAI/. Use `./`, `../` or an absolute path for a local source.

The spec must contain a valid manifest, lock and spec/ directory. See [the manifest format](docs/manifest.md). Relative repository URLs resolve against the spec source. The three repository directory names must be distinct.

Existing destinations are preserved. Fetching the spec uses a temporary .cspec-clone-* directory in the destination parent. Success leaves only the project root. Failure reports and preserves any new directories for inspection.

## Work inside a project

```sh
cspec project info
cspec project open --print
cspec guidelines path
cspec guidelines edit
```

These commands find the spec from the project root or inside its code, spec or guidelines repository. You can also pass a project or spec directory. Multiple spec candidates require an explicit spec path.

`project info` reads the current definition, verifies repository origins and prepares any missing guidelines clone or snapshot. Existing editable clones are not automatically pulled or reset.

`project open` generates <spec>/.local/project.code-workspace and opens it using the operating system's file association. `--print` generates the file and prints its path without launching an editor. VS Code supports this format; other editors can open the directories reported by project info.

Editor folders are derived from the current spec; other workspace settings are preserved. Removing the editor file does not lose project configuration. Moving the whole project preserves the relationships between all three repositories.

`guidelines edit` opens this project's editable CretAI clone. Each project has its own clone pointing to the shared CretAI remote. Commit and push improvements there; fetch them in other projects when needed. Local drafts do not change another project's files or its selected guidelines.

To adopt new guidelines, update the spec's manifest and lock together. Fetch the selected commit and tag in that project's CretAI clone with `git fetch --tags`, then run project info or project open. Tags and exact commits must agree.

## Existing clones and upgrading from 0.2

Place the repositories inside one ordinary project root:

```sh
cspec project attach /projects/Example/Example /projects/Example/Example-spec
```

Attach verifies the existing code and spec and clones the guidelines if missing. It preserves tracked files and stores no separate path bindings.

Version 0.3 replaces the flat 0.2 layout and its globally selected guidelines clone. Keep old trials as backups or move the three repositories into a project root. Set `cspec config namespace <owner>` to replace the old personal guidelines-path setting, even if that clone has moved. The optional clone destination now means the project root.

## Limits and development

- The tool, guidelines and product have independent versions.
- Snapshots must match the lock and have no local changes. They are detached checkouts, not filesystem-enforced read-only directories.
- Clone does not install product dependencies or execute project scripts.
- Skills and assistant integrations are not activated automatically.
- Spec generation, Spec Kit integration, standalone installers and automated releases remain future work.

See [CONTRIBUTING.md](CONTRIBUTING.md). CI checks syntax, integration scenarios, launchers and packaging on Windows, Linux and macOS with Node.js 22 and 24.
