---
title: Troubleshooting
description: Diagnose project layout, authentication, lock and installation problems.
---

Start with `cspec project doctor`. It checks the project without fetching or generating files. Add `--json` for structured check results; a failed check exits with status 1.

| Problem | Next step |
| --- | --- |
| `cspec` not found or wrong version | Check your PATH and [installation conflicts](/CretSpec/start/install/#replace-the-earlier-nodejs-installation). |
| Git cannot be started | Install Git, open a new terminal and check `git --version`. |
| Clone authentication fails | Run `git ls-remote` with the same source and fix Git's credentials. |
| No namespace configured | Set `cspec config namespace <owner>` or use a full source URL. |
| Destination already exists | Choose a new outer destination, or attach correctly arranged existing clones. |
| No spec or multiple specs found | Pass the intended spec directory explicitly. |
| Code or guidelines source mismatch | Inspect Git origins and the manifest. Correct the intended definition or use the right clone. |
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
| Duplicate skills shown by an editor | Select the integrations you use in `project.json`; compatibility readers can see both directories. Open one repository to narrow multi-root discovery. |
| Skill not visible in an agent | Run doctor, check the selected integration and the host's skills/trust settings, then start a fresh agent session. Doctor checks files, not the running agent. |
| Shared skill is only listed as a draft | Commit it in the editable guidelines repo and explicitly adopt that guidelines version. |
| Another synchronization is running | Wait for that process to finish. The operating-system lock releases on process exit. |

## Failed cloning or initialization

CretSpec preserves partial new project directories and prints their location when cloning fails. Inspect them before removing anything. It will not overwrite them on retry; use a new destination or reconcile the partial setup first.

Initialization validates its inputs before writing. If a filesystem failure interrupts writing, newly created files are preserved and the command reports the interruption. Inspect `project.json`, `guidelines.lock.json` and `spec/` before retrying.

## Report a problem

Include `cspec --version`, operating system, the command, error text and the smallest reproducible example. Remove private repository names, credentials and confidential spec content. Public issues should not contain your complete personal configuration or private project definition.
