#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { cloneProject, attachProject, workspacePath, openPath } from '../src/workspace.mjs';
import { getGuidelines, setGuidelines } from '../src/config.mjs';

const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const help = `CretSpec ${version} — code, spec en ontwikkelafspraken in één werkruimte

Gebruik:
  cspec guidelines set <pad-naar-CretAI>
  cspec guidelines path
  cspec guidelines edit [--print]
  cspec project clone <spec-repository> <nieuwe-werkmap>
  cspec project attach <codepad> <specpad> <nieuwe-werkmap>
  cspec project open [werkmap] [--print]
  cspec --version
  cspec --help

Git en Node.js 22+ zijn nodig. Stel eerst je richtlijnenmap in.
open en edit gebruiken de standaardapplicatie van je besturingssysteem.
Clonen installeert of start geen projectcode.
`;

try {
  const args = process.argv.slice(2);
  if (args.length === 0 || (args.length === 1 && ['--help', '-h'].includes(args[0]))) {
    console.log(help);
  } else if (args.length === 1 && ['--version', '-v'].includes(args[0])) {
    console.log(version);
  } else if (args[0] === 'guidelines' && args[1] === 'set' && args.length === 3) {
    console.log(`Bewerkbare richtlijnen: ${await setGuidelines(args[2])}`);
  } else if (args[0] === 'guidelines' && args[1] === 'path' && args.length === 2) {
    console.log(await getGuidelines());
  } else if (args[0] === 'guidelines' && args[1] === 'edit' && (args.length === 2 || (args.length === 3 && args[2] === '--print'))) {
    const directory = await getGuidelines();
    console.log(directory);
    if (args.length === 2) await openPath(directory);
  } else if (args[0] === 'project' && args[1] === 'clone' && args.length === 4) {
    const result = await cloneProject(args[2], args[3], {
      guidelinesRoot: await getGuidelines(), progress: message => console.log(`• ${message}`),
    });
    console.log(`\n${result.manifest.name} is klaar.\nWerkruimte: ${result.workspaceFile}\nRichtlijnen: ${result.lock.ref} (${result.lock.commit.slice(0, 12)})`);
  } else if (args[0] === 'project' && args[1] === 'attach' && args.length === 5) {
    const result = await attachProject(args[2], args[3], args[4], {
      guidelinesRoot: await getGuidelines(), progress: message => console.log(`• ${message}`),
    });
    console.log(`Werkruimte gekoppeld: ${result.workspaceFile}`);
  } else if (args[0] === 'project' && args[1] === 'open') {
    const tail = args.slice(2);
    const print = tail.includes('--print');
    const paths = tail.filter(arg => arg !== '--print');
    if (paths.length > 1 || paths.some(arg => arg.startsWith('-')) || tail.filter(arg => arg === '--print').length > 1) throw new Error(help);
    const filename = await workspacePath(paths[0]);
    console.log(filename);
    if (!print) await openPath(filename);
  } else {
    throw new Error(`Onbekende opdracht of onjuiste argumenten.\n\n${help}`);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
