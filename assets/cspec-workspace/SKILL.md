---
name: cspec-workspace
description: Work in a CretSpec workspace, inspect its specification and adopted guidelines, maintain requirements and stories, or save a reusable approach as a project or shared skill. Use when a request concerns the workspace method, its guidance, or the cspec CLI.
---

# CretSpec workspace

Run `cspec context --json` from the workspace or a sibling repository. Its versioned output identifies the spec, code, editable guidelines, exact active snapshot, instruction sources and skill source directories. Read the relevant sources; do not infer repository roles from names. If context is unavailable, use `cspec project doctor --json` and explain the reported problem before changing configuration.

Apply the adopted specification method and selected technology profile. Keep concrete requirements, milestones, stories, decisions and verification evidence in the spec. Record checks actually performed and distinguish proposed work from agreed scope. The CLI does not determine whether a story is complete.

## Preserve reusable knowledge

When asked to remember an approach, first decide whether it is an agreement, a project decision or a repeatable skill. An agreement belongs in the appropriate guidelines or project rules; it does not automatically need a skill.

- Project-specific skills belong in the spec's `spec/skills/<name>/`.
- Reusable skills belong in the editable guidelines repository's `skills/<name>/`.
- If the intended scope is already clear, use it. Otherwise ask one concise question: should this be specific to this project or shared across projects? Explain the proposed scope using the user's example.

Use `cspec skill list --json` to inspect active skills and editable shared drafts. Use `cspec skill create <name> --scope project|shared --description "What it does and when to use it" --json` to scaffold the selected source. Complete the returned `SKILL.md` and include only supporting resources the skill needs. Keep the name, description and relative resource links portable across agents. Check any script's Windows, Linux and macOS dependencies; CretSpec does not run scripts during preparation.

Run `cspec sync` after changing project skill sources. Shared drafts become active only after normal Git publication and explicit project adoption with `cspec guidelines update <reference>`. Creating a shared skill does not authorize publishing it or changing other projects. Never edit the pinned snapshot or generated skill copies.

## CLI use

Use `cspec --version` and command-specific `--help` for the installed interface. `context`, `skill list` and `project doctor` are read-only. `sync` rebuilds local integrations without fetching or adopting guidance. `project info` can prepare missing snapshots. `guidelines edit` locates the editable shared repository.

If synchronization reports a conflict, preserve the user's file and explain which source or generated path needs reconciliation. Do not force-overwrite it. If an update saved a new definition but integration refresh failed, resolve the conflict and run `cspec sync`.

Generated entry points and skill copies are local conveniences. Edit their sources, keep private development context out of product commits, and respect existing repository instructions. Agent discovery and execution permissions are controlled by the host; generated files do not bypass trust or grant access to sibling directories.
