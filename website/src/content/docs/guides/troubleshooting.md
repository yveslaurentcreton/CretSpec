---
title: Troubleshooting
description: Diagnose project layout, authentication, lock and installation problems.
---

Start with `cspec project doctor`. It checks the project without fetching or generating files. Add `--json` for structured check results; a failed check exits with status 1.

| Problem | Next step |
| --- | --- |
| `cspec` not found or wrong version | Check your PATH and follow the [installation checks](/CretSpec/start/install/#check-your-installation). |
| Git cannot be started | Install Git, open a new terminal and check `git --version`. |
| Clone authentication fails | Run `git ls-remote` with the same source and fix Git's credentials. |
| No namespace configured | Set `cspec config namespace <owner>` or use a full source URL. |
| Destination already exists | Choose a new outer destination, or attach correctly arranged existing clones. |
| No spec or multiple specs found | Pass the intended spec directory explicitly. |
| Code or guidelines source mismatch | Inspect Git origins and the manifest. Correct the intended definition or use the right clone. |
| Declared repository missing | Clone it with Git at its declared path, or run `cspec project attach <spec> --clone-missing`. |
| Undeclared repository warning | Declare the repository in `project.json` if it belongs to the workspace, or move it out of the project root. |
| Origin contains a password | Remove it with `git remote set-url` and use your Git credential manager. A user name alone is accepted. |
| Existing `spec/` stops initialization | Add `--existing-spec` to keep it and write only `project.json`. |
| Invalid or unknown role | Check the role file's frontmatter, repository names and skill names, then run `cspec sync`. |
| Role cannot write to a repository | The role folder is the agent's working directory. Add the focus repositories as additional directories in your host; `cspec context --role <name>` lists them. |
| Claude Code ignores the workspace instructions | Doctor names the `CLAUDE.md`, `.claude/CLAUDE.md` or `CLAUDE.local.md` that makes it skip `AGENTS.md`. Commit only `AGENTS.md`, or set Claude Code's **Project instructions** to `claude-md-and-agents-md`. Claude Code 2.1.277 or later is required. |
| Claude Code shows skills a role did not select | Claude Code also loads skills from the directories above the role folder. Omit `skills` in the role, or accept the extra skills. |
| Codex instructions exceed 32 KiB | Shorten the repository's `AGENTS.md` or the role instructions, or raise Codex's `project_doc_max_bytes`. |
| Local context or snapshot missing | Run `cspec project info` to prepare it. |
| Locked commit unavailable | Fetch tags in the editable guidelines clone. Check that the locked ref was published. |
| Ref differs from the lock | Investigate a moved tag or inconsistent manifest/lock. Restore the agreed pair before adopting a different version. |
| Cached snapshot modified | Preserve any work in that snapshot, then remove only that snapshot and run `project info`. |
| `.local/` contains tracked files | Preserve needed content, remove it from Git's index, and keep `.local/` ignored. |
| Interrupted update | Run `cspec guidelines recover`; see [recovery](/CretSpec/guides/guidelines/#recover-an-interrupted-update). |
| Editor does not open | Use `project open --print`, then open the printed path manually. Linux desktop opening requires `xdg-open`. |
| Missing or stale agent integrations | Run `cspec sync`; missing snapshots first require `cspec project info`. |
| Edited or unmanaged generated file | Preserve its changes in the source or another file, remove only the reported generated output, then run `cspec sync`. |
| Tracked generated path | Remove that generated file from Git's index while preserving the source. Do not publish private context with the product. |
| Interrupted synchronization | Keep `.local/agents-pending.json` and retry `cspec sync`; user edits are not overwritten. |
| Duplicate skill names in source repositories | Rename one source skill. Project and shared active names must be distinct. |
| A skill overrides another | A further guidelines reference or a referenced project defines the same name. The source with the highest precedence wins; rename one, or change `priority` if the other should win. |
| Typed reference fields require schemaVersion 3 | Run `cspec project migrate`, then add the references. |
| Reference cycle | A project references itself, directly or through other projects. Remove one reference in the named path. |
| A referenced repository is missing | Run `cspec project attach <spec> --clone-missing`. Check access with `git ls-remote` when cloning fails; the rest of the workspace keeps working. |
| Two references want different guidelines branches | One working tree serves the workspace; the reference closest to the root decides. Pin one with `commit`, or align the branches. |
| Duplicate skills shown by an editor | Select the integrations you use in `project.json`; compatibility readers can see both directories. Open one repository to narrow multi-root discovery. |
| Skill not visible in an agent | Run doctor, check the selected integration and the host's skills/trust settings, then start a fresh agent session. Doctor checks files, not the running agent. |
| Shared skill is only listed as a draft | The project is pinned. Commit and explicitly adopt the version, or use `guidelines unlock` to use local sources. |
| Working-tree guidelines are behind | Use Git fetch and pull in the guidelines clone, then `cspec sync`. Counts reflect cached refs. |
| Dirty, detached or unexpected guidelines branch | Local contents remain active. Review them and select the intended branch using Git. |
| Explicit pinned mode has no lock | Restore the original lock, then use `guidelines unlock` if you intend to switch modes. |
| Working-tree ref is a tag or commit | Restore a valid branch definition first; then use `guidelines update` if you want to pin that version. |
| Another synchronization is running | Wait for that process to finish. The operating-system lock releases on process exit. |

## Failed cloning or initialization

CretSpec preserves partial new project directories and prints their location when cloning fails. Inspect them before removing anything. It will not overwrite them on retry; use a new destination or reconcile the partial setup first.

Initialization validates its inputs before writing. If a filesystem failure interrupts writing, newly created files are preserved and the command reports the interruption. Inspect `project.json`, `guidelines.lock.json` and `spec/` before retrying.

## Report a problem

Include `cspec --version`, operating system, the command, error text and the smallest reproducible example. Remove private repository names, credentials and confidential spec content. Public issues should not contain your complete personal configuration or private project definition.
