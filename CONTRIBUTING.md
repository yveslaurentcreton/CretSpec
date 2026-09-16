# Contributing to CretSpec

Use Git and Node.js 22 or later. There are no additional runtime dependencies.

```sh
npm run check
npm test
```

Tests use temporary local Git repositories. CI covers Windows, Linux and macOS with Node.js 22 and 24. Tests must not use personal configuration: set a temporary CRETSPEC_HOME.

Write documentation, comments, examples and CLI messages in English.

For development, run `node bin/cspec.mjs` or use `npm link`. A packaged installation must work without its own Git checkout. CretSpec's own source may use the same project layout as any other product; other projects do not need a copy of it. Resolve editable guidelines from the project's spec and its local sibling repositories, never from the tool installation or a globally selected working copy.

Keep the spec as the only source of project configuration. Generated editor files and caches belong inside the spec's ignored `.local/` directory and must be rebuildable from the current manifest and lock.

Pass Git commands as argument arrays without shell interpolation. Do not overwrite existing clone destinations or execute scripts from a project spec. Failures may leave partial new files, but must preserve existing source content.

Changes to manifest fields require matching validation, schemas and documentation. Add tests for meaningful user scenarios when changing behavior.
