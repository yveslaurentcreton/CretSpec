# Project definition

The spec repository is the only authoritative project definition. The ordinary project root only groups three independent repositories.

```text
Example/
  Example-spec/
    project.json
    guidelines.lock.json
    spec/...
    .local/                     generated, ignored
  Example/                      code
  CretAI/                       editable guidelines
```

## project.json

```json
{
  "schemaVersion": 1,
  "name": "Example",
  "code": { "repository": "../Example" },
  "guidelines": { "repository": "../CretAI", "ref": "v0.3.0" },
  "profile": "dotnet"
}
```

The name determines the default project root and its code directory. An explicit clone destination overrides only the root's local name. The spec and guidelines directories use their repository basenames without .git. All three names must be distinct, ignoring case.

A project name starts with a letter and uses letters, digits, dots, underscores or hyphens. Trailing dots and Windows device names are rejected.

code.repository identifies the product source. guidelines.repository identifies shared guidance. guidelines.ref selects its version, preferably an immutable tag. profile selects profiles/<profile>.md in that version.

Relative references resolve against the **spec source**: for git@github.com:owner/Example-spec.git, ../Example means git@github.com:owner/Example. Use full HTTPS or SSH URLs for different owners or servers. Remote specs cannot reference absolute local repositories. Local trials can use local paths.

## guidelines.lock.json

```json
{
  "schemaVersion": 1,
  "ref": "v0.3.0",
  "commit": "FULL_COMMIT_ID_OF_THE_SELECTED_TAG"
}
```

Replace the placeholder with the full result of `git rev-parse "v0.3.0^{commit}"` in CretAI. CretSpec verifies the ref and exact commit and prepares a detached snapshot. The editable CretAI clone remains separate on its normal branch.

This lock selects guidelines, not the tool installation or the code and spec revisions. Record implemented product commits or releases in the private spec when needed.

## Discovery and generated files

The configured repository namespace resolves a short spec name before cloning. It holds no project details. Once cloned, commands discover the spec from the project root or a repository within it, without consulting personal settings. An explicit spec path resolves ambiguity.

Only the spec's ignored .local/ directory receives snapshots and optional editor files. There is no outer marker, duplicate manifest or project registry. Editor folders are rebuilt from the current spec; other editor settings are preserved.

Moving the whole root preserves relationships among code, spec and editable guidelines. Their Git origins still identify their sources. Attaching existing clones requires the expected sibling names and matching origins.

## Schemas

[project.schema.json](../schemas/project.schema.json) and [guidelines-lock.schema.json](../schemas/guidelines-lock.schema.json) describe schemaVersion 1. Unknown fields are rejected. Runtime checks also enforce valid repository references, portable names and exact version agreement.
