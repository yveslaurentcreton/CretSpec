#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { cloneProject, attachProject, workspacePath, projectInfo, openPath } from '../src/workspace.mjs';
import { getNamespace, setNamespace } from '../src/config.mjs';

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const help = `CretSpec ${version} — one root directory per project

Usage:
  cspec config namespace [GitHub-owner-or-namespace-URL]
  cspec project clone <spec-name-or-url> [project-directory]
  cspec project attach <code-directory> <spec-directory>
  cspec project info [project-or-spec-directory]
  cspec project open [project-or-spec-directory] [--print]
  cspec guidelines path [project-or-spec-directory]
  cspec guidelines edit [project-or-spec-directory] [--print]
  cspec --version
  cspec --help

Example:
  cspec config namespace your-github-owner
  cspec project clone CretQL-spec

Clone reads the spec first, then creates CretQL/ containing CretQL-spec/,
CretQL/ and CretAI/. The spec owns the project definition.
CretSpec is installed separately and is not copied into projects.
Use ./ or an absolute path for a local source repository.

Git and Node.js 22+ are required. Clone does not install or run project code.
Editor files are generated only by project open, inside the spec's .local/.
`;

function locationAndPrint(args) {
  const paths = args.filter(arg => arg !== '--print');
  if (paths.length > 1 || paths.some(arg => arg.startsWith('-')) || args.filter(arg => arg === '--print').length > 1) throw new Error(help);
  return { location: paths[0], print: args.includes('--print') };
}

try {
  const args = process.argv.slice(2);
  if (args.length === 0 || (args.length === 1 && ['--help', '-h'].includes(args[0]))) {
    console.log(help);
  } else if (args.length === 1 && ['--version', '-v'].includes(args[0])) {
    console.log(version);
  } else if (args[0] === 'config' && args[1] === 'namespace' && [2, 3].includes(args.length)) {
    const namespace = args.length === 3 ? await setNamespace(args[2]) : await getNamespace();
    if (!namespace) throw new Error('No repository namespace configured. Use cspec config namespace <GitHub-owner-or-namespace-URL>.');
    console.log(namespace);
  } else if (args[0] === 'guidelines' && args[1] === 'path' && [2, 3].includes(args.length)) {
    if (args[2]?.startsWith('-')) throw new Error(help);
    console.log((await projectInfo(args[2])).editableGuidelines);
  } else if (args[0] === 'guidelines' && args[1] === 'edit') {
    const { location, print } = locationAndPrint(args.slice(2));
    const directory = (await projectInfo(location)).editableGuidelines;
    console.log(directory);
    if (!print) await openPath(directory);
  } else if (args[0] === 'project' && args[1] === 'clone' && [3, 4].includes(args.length)) {
    if (args.slice(2).some(arg => arg.startsWith('-'))) throw new Error(help);
    const shortName = /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(args[2]);
    const result = await cloneProject(args[2], args[3], {
      repositoryNamespace: shortName ? await getNamespace() : undefined,
      progress: message => console.log(`• ${message}`),
    });
    console.log(`\n${result.manifest.name} is ready.\nProject: ${result.projectRoot}\nSpec: ${result.spec}\nCode: ${result.code}\nEditable guidelines: ${result.editableGuidelines}\nActive guidelines: ${result.lock.ref} (${result.lock.commit.slice(0, 12)})`);
  } else if (args[0] === 'project' && args[1] === 'attach' && args.length === 4) {
    const result = await attachProject(args[2], args[3], { progress: message => console.log(`• ${message}`) });
    console.log(`Project ready: ${result.projectRoot}`);
  } else if (args[0] === 'project' && args[1] === 'info' && [2, 3].includes(args.length)) {
    if (args[2]?.startsWith('-')) throw new Error(help);
    console.log(JSON.stringify(await projectInfo(args[2]), null, 2));
  } else if (args[0] === 'project' && args[1] === 'open') {
    const { location, print } = locationAndPrint(args.slice(2));
    const filename = await workspacePath(location);
    console.log(filename);
    if (!print) await openPath(filename);
  } else {
    throw new Error(`Unknown command or invalid arguments.\n\n${help}`);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
