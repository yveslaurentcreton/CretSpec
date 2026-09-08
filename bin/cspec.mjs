#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { cloneProject, attachProject, workspacePath, projectInfo, openPath } from '../src/workspace.mjs';
import { getGuidelines, setGuidelines } from '../src/config.mjs';

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const help = `CretSpec ${version} — start with the spec

Usage:
  cspec guidelines set <path-to-CretAI>
  cspec guidelines path
  cspec guidelines edit [--print]
  cspec project clone <spec-name-or-url> [spec-directory]
  cspec project attach <code-directory> <spec-directory>
  cspec project info [spec-directory]
  cspec project open [spec-directory] [--print]
  cspec --version
  cspec --help

Example:
  cspec project clone CretQL-spec

A short name uses the namespace of your configured guidelines repository.
The spec is cloned first. Its project.json determines the code repository
and sibling directory; its lock selects the exact guidelines version.
Use ./ or an absolute path for a local source repository.

Git and Node.js 22+ are required. Clone does not install or run project code.
Editor files are generated only by project open, inside the spec's .local/.
`;

try {
  const args = process.argv.slice(2);
  if (args.length === 0 || (args.length === 1 && ['--help', '-h'].includes(args[0]))) {
    console.log(help);
  } else if (args.length === 1 && ['--version', '-v'].includes(args[0])) {
    console.log(version);
  } else if (args[0] === 'guidelines' && args[1] === 'set' && args.length === 3) {
    console.log(`Editable guidelines: ${await setGuidelines(args[2])}`);
  } else if (args[0] === 'guidelines' && args[1] === 'path' && args.length === 2) {
    console.log(await getGuidelines());
  } else if (args[0] === 'guidelines' && args[1] === 'edit' && (args.length === 2 || (args.length === 3 && args[2] === '--print'))) {
    const directory = await getGuidelines();
    console.log(directory);
    if (args.length === 2) await openPath(directory);
  } else if (args[0] === 'project' && args[1] === 'clone' && [3, 4].includes(args.length)) {
    if (args.slice(2).some(arg => arg.startsWith('-'))) throw new Error(help);
    const result = await cloneProject(args[2], args[3], {
      guidelinesRoot: await getGuidelines(), progress: message => console.log(`• ${message}`),
    });
    console.log(`\n${result.manifest.name} is ready.\nSpec: ${result.spec}\nCode: ${result.code}\nGuidelines: ${result.lock.ref} (${result.lock.commit.slice(0, 12)})`);
  } else if (args[0] === 'project' && args[1] === 'attach' && args.length === 4) {
    const result = await attachProject(args[2], args[3], {
      guidelinesRoot: await getGuidelines(), progress: message => console.log(`• ${message}`),
    });
    console.log(`Project ready. Spec: ${result.spec}\nCode: ${result.code}`);
  } else if (args[0] === 'project' && args[1] === 'info' && [2, 3].includes(args.length)) {
    if (args[2]?.startsWith('-')) throw new Error(help);
    console.log(JSON.stringify(await projectInfo(args[2], { guidelinesRoot: await getGuidelines() }), null, 2));
  } else if (args[0] === 'project' && args[1] === 'open') {
    const tail = args.slice(2);
    const print = tail.includes('--print');
    const paths = tail.filter(arg => arg !== '--print');
    if (paths.length > 1 || paths.some(arg => arg.startsWith('-')) || tail.filter(arg => arg === '--print').length > 1) throw new Error(help);
    const filename = await workspacePath(paths[0], { guidelinesRoot: await getGuidelines() });
    console.log(filename);
    if (!print) await openPath(filename);
  } else {
    throw new Error(`Unknown command or invalid arguments.\n\n${help}`);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
