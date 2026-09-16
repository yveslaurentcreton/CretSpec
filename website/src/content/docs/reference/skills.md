---
title: Skills and agent context
description: Skill source layout, validation, generated files and structured CLI output.
---

CretSpec uses the [Agent Skills format](https://agentskills.io/specification). Each skill is a directory named after its `name`, containing `SKILL.md` and optional resources:

```text
spec/skills/query-validation/
├── SKILL.md
└── references/
    └── examples.json
```

```md
---
name: query-validation
description: Validate query inputs and expected results for this project.
---

Use the cases in [examples](references/examples.json) to check the query behavior.
Record the observed results and unresolved cases in the specification.
```

The description explains when to use the skill. Detailed instructions and links to supporting files belong in the body. Keep relative resource references inside the skill bundle. Host-specific optional YAML fields are preserved; CretSpec does not translate their semantics or guarantee support in every agent.

## Sources and validation

- Project skills: `<spec>/spec/skills/<name>/`, read from the working tree.
- Shared skill drafts: `<editable-guidelines>/skills/<name>/`.
- Adopted shared skills: `<spec>/.local/guidelines/<commit>/skills/<name>/`.
- Built-in `cspec-workspace`: embedded in the executable, reserved and versioned with CretSpec.

Names contain 1–64 lowercase ASCII letters, digits and single hyphens, with no leading/trailing hyphen or Windows device name. The YAML `name` must match its directory. The description must be nonempty and at most 1024 characters. UTF-8 and CRLF are accepted, including folded YAML descriptions. Duplicate active names across project and shared sources are rejected instead of silently overriding one another.

The source directory may contain a `README.md` catalog alongside its skill directories. Each skill directory must contain `SKILL.md`. Resource names start with an ASCII letter/digit and contain only letters, digits, `.`, `_` or `-`; hidden names, device names, trailing dots and names differing only by case are rejected. Resources must be regular files and directories, with no symlinks. Limits are 128 KiB per `SKILL.md`, 256 files and 16 MiB per skill, eight nested directories, 128 skills per source and 64 MiB of active resources in total.

Full bundles are copied, including references, assets and scripts. Unix executable permissions are preserved. Scripts are never executed during clone, sync or skill creation. Declare platform dependencies and use cross-platform commands when intended for all supported operating systems.

An invalid shared draft is reported separately by `skill list` and does not invalidate adopted skills. A requested guidelines update validates its proposed active skill set before changing the project definition.

## Generated files

Selected integrations are generated at the outer workspace, code, spec and editable guidelines roots. Paths in instructions are relative to their corresponding root. Moving the whole workspace keeps them usable; no generated absolute project binding exists.

When an existing `AGENTS.md` is present, its content is included in a generated `AGENTS.override.md`. The original is unchanged. Existing instructions have a 24 KiB input limit; larger inputs need consolidation. Existing `CLAUDE.md` is kept and accompanied by `CLAUDE.local.md`. A pre-existing conflicting local/override destination is never adopted or overwritten automatically.

The spec's ignored `.local/` contains:

| File | Purpose |
| --- | --- |
| `agents-state.json` | Owned output paths and SHA-256/permission fingerprints |
| `agents-pending.json` | Present during synchronization; permits safe retry after interruption |
| `agents.lock` | Operating-system lock preventing concurrent synchronizations; released when the process exits |

Synchronization validates sources and conflicts before changing outputs, replaces individual files atomically, then commits ownership metadata. It is not one atomic filesystem transaction across all repositories. If interrupted, run `cspec sync` again. A pending journal and manual edits are preserved when they conflict. Do not delete ownership metadata while generated files exist: without it, CretSpec treats those files as unmanaged.

When a source skill disappears or integrations are disabled, only unchanged files recorded as owned are removed. Unrelated resources and directories are preserved. Local Git exclusion entries may remain and are harmless. A completed guidelines adoption remains adopted if a later integration refresh fails; reconcile the reported file and run `cspec sync`.

## Structured output

The new JSON responses have `schemaVersion: 1`. Errors go to stderr and command failures return a nonzero status. Optional future JSON fields may be added; consumers should read the fields they need.

| Command | Top-level fields |
| --- | --- |
| `context --json` | `schemaVersion`, `cliVersion`, `project`, `instructionSources`, `skillSources`, `skills`, `integration`, `compatibilityNotice` |
| `skill list --json` | `schemaVersion`, `active`, `sharedDrafts`, `draftErrors` |
| `skill create ... --json` | `schemaVersion`, `name`, `scope`, `source`, `nextStep` |
| `sync --json` | `schemaVersion`, `written`, `removed`, `unchanged`, `compatibilityNotice` |

`project` has the same fields as `project info`. `skillSources` has `project`, `shared` and `activeShared` paths. Skills have `name`, `description`, `scope` (`project`, `shared` or `builtin`) and `source`; the built-in source is `null` because it is embedded. Shared working-copy entries may match adopted skills; they describe an editable source, not an additional active skill.

`integration` contains `ready` and `detail`. A context report can succeed with `ready: false`: the sources are readable but generated context needs attention. Use `project doctor --json` when a readiness failure must produce exit status 1. Filesystem access or invalid source errors still fail `context` itself.
