---
title: Roles
description: Give an agent the shared project context with a specific focus.
---

A role adds a focus to the shared context, such as operations or application development. Most workspaces do not need roles: project rules and skills are usually enough.

## Define roles

Store each role in the specification as `spec/roles/<name>.md`:

```md
---
name: app-dev
description: Develop and test the application, including related infrastructure changes.
repositories: [atlas, infra]
---

Develop features through the application's own specification workflow.
Keep application code and infrastructure changes in their own repositories.
Check the infrastructure repository's branch and status before editing it.
```

`repositories` lists the focus repositories by name, in priority order. The body describes what the role does and what it leaves to other roles. Omit the optional `skills` field to give the role every skill of the workspace.

Run `cspec sync`. Every role gets its own folder:

```text
roles/app-dev/
├── AGENTS.md            shared context plus the role
├── .agents/skills/      the role's skills
└── .claude/skills/
```

## Select a role

Agents take their instructions from the folder in which they start, so start the agent in the role's folder. Two conversations can use different roles at the same time:

```sh
cd Atlas/roles/operations && codex
cd Atlas/roles/app-dev && claude
```

In a desktop agent app that organizes work by folder, register each role folder as its own project. To give a role to a conversation that started elsewhere, ask the agent to run:

```sh
cspec context --role app-dev
```

This prints the role's instructions, focus repositories and skills without changing any file.

The repositories a role works on are siblings of its folder. A host that only allows writes inside its working directory needs them as additional directories; `cspec context --role <name>` lists them. For example, with Claude Code:

```sh
claude --add-dir ../../atlas --add-dir ../../atlas-infra
```

:::note
Roles are working agreements, not a security boundary. Agents running as the same operating-system user can read the same files and use the same credentials. Use your host's approval settings or separate accounts when you need enforcement.
:::

## Limits

- A `skills` selection is not isolation: Claude Code also loads `.claude/skills/` from the folders above its working directory.
- Role folders are generated. Edit the role source and run `cspec sync`; removing a role removes only the files CretSpec created for it.
- A referenced project's own roles are not generated. Define roles in your own specification.
- Workspace and repository entry points never contain role instructions, so a role cannot leak into another conversation.

The [role file reference](/CretSpec/reference/skills/#role-files) lists every field and limit.
