import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { normalizeNamespace, namespaceFromRepository, originOrLocal, repositoryRoot } from './workspace.mjs';

export function configPath() {
  return path.join(path.resolve(process.env.CRETSPEC_HOME || path.join(os.homedir(), '.cretspec')), 'config.json');
}

async function readConfig() {
  const filename = configPath();
  try {
    const info = await fs.lstat(filename);
    if (!info.isFile() || info.size > 128 * 1024) throw new Error('Invalid configuration file.');
    const data = JSON.parse(await fs.readFile(filename, 'utf8'));
    const current = data?.schemaVersion === 2 && typeof data.repositoryNamespace === 'string';
    const legacy = data?.schemaVersion === 1 && typeof data.guidelinesRoot === 'string';
    if (!current && !legacy) throw new Error('Invalid CretSpec configuration.');
    return data;
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw new Error(`Cannot read ${filename}: ${error.message}`);
  }
}

export async function setNamespace(value) {
  const repositoryNamespace = normalizeNamespace(value);
  const previous = await readConfig();
  const { guidelinesRoot: _legacyPath, ...settings } = previous || {};
  const filename = configPath();
  await fs.mkdir(path.dirname(filename), { recursive: true });
  await fs.writeFile(filename, JSON.stringify({ ...settings, schemaVersion: 2, repositoryNamespace }, null, 2) + '\n');
  return repositoryNamespace;
}

export async function getNamespace() {
  const config = await readConfig();
  if (!config) return null;
  if (config.schemaVersion === 2) return normalizeNamespace(config.repositoryNamespace);
  try {
    return namespaceFromRepository(originOrLocal(repositoryRoot(config.guidelinesRoot)));
  } catch {
    throw new Error('The old configuration points to a guidelines clone that has moved. Set the repository namespace with: cspec config namespace <GitHub-owner-or-namespace-URL>');
  }
}
