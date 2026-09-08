import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { repositoryRoot } from './workspace.mjs';

export function configPath() {
  return path.join(path.resolve(process.env.CRETSPEC_HOME || path.join(os.homedir(), '.cretspec')), 'config.json');
}

async function validateGuidelines(directory) {
  const root = repositoryRoot(await fs.realpath(path.resolve(directory)));
  const principles = await fs.lstat(path.join(root, 'guidelines', 'principles.md'));
  const profiles = await fs.lstat(path.join(root, 'profiles'));
  if (!principles.isFile() || !profiles.isDirectory()) {
    throw new Error('Kies een richtlijnenrepository met guidelines/principles.md en profiles/.');
  }
  return root;
}

async function readConfig() {
  const filename = configPath();
  try {
    const info = await fs.lstat(filename);
    if (!info.isFile() || info.size > 128 * 1024) throw new Error('Ongeldig configuratiebestand.');
    const data = JSON.parse(await fs.readFile(filename, 'utf8'));
    if (data?.schemaVersion !== 1 || typeof data.guidelinesRoot !== 'string' || !path.isAbsolute(data.guidelinesRoot)) {
      throw new Error('Ongeldige CretSpec-configuratie.');
    }
    return data;
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw new Error(`Kan ${filename} niet lezen: ${error.message}`);
  }
}

export async function setGuidelines(directory) {
  const guidelinesRoot = await validateGuidelines(directory);
  const previous = await readConfig();
  const filename = configPath();
  await fs.mkdir(path.dirname(filename), { recursive: true });
  await fs.writeFile(filename, JSON.stringify({ ...previous, schemaVersion: 1, guidelinesRoot }, null, 2) + '\n');
  return guidelinesRoot;
}

export async function getGuidelines() {
  const config = await readConfig();
  if (!config) throw new Error('Stel je bewerkbare richtlijnenmap eenmalig in met: cspec guidelines set <pad-naar-CretAI>');
  try { return await validateGuidelines(config.guidelinesRoot); }
  catch (error) {
    throw new Error(`De ingestelde richtlijnenmap is niet bruikbaar: ${config.guidelinesRoot}\n${error.message}\nGebruik cspec guidelines set <nieuw-pad> na het verplaatsen of hernoemen.`);
  }
}
