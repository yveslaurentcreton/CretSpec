import { execFileSync, spawn } from 'node:child_process';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function git(args, cwd) {
  try {
    return execFileSync('git', ['-c', 'protocol.ext.allow=never', ...args], {
      cwd, encoding: 'utf8', windowsHide: true,
      stdio: ['inherit', 'pipe', 'pipe'], maxBuffer: 8 * 1024 * 1024,
    }).trim();
  } catch (error) {
    if (error.code === 'ENOENT') throw new Error('Git is niet gevonden. Installeer Git en controleer je PATH.');
    const detail = String(error.stderr || error.message).replace(/(https?:\/\/)[^\s/@]+@/g, '$1[redacted]@');
    throw new Error(`Git-opdracht mislukt: ${detail.trim()}`);
  }
}

function repositoryKind(value) {
  if (typeof value !== 'string' || !value.trim() || /[\r\n\0]/.test(value)) {
    throw new Error('Een repositoryverwijzing moet een niet-lege URL of pad zijn.');
  }
  if (/^[A-Za-z][A-Za-z0-9+.-]*::/.test(value)) throw new Error('Git remote helpers zijn niet toegestaan.');
  if (/^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(value)) {
    const url = new URL(value);
    if (!['https:', 'ssh:', 'file:'].includes(url.protocol)) throw new Error('Gebruik HTTPS, SSH of een lokaal Git-pad.');
    if (url.password || (url.protocol === 'https:' && url.username) || url.search || url.hash) {
      throw new Error('Gebruik geen credentials, queryparameters of fragmenten in repository-URLs.');
    }
    return url.protocol === 'file:' ? 'file' : 'url';
  }
  if (/^[^\s/@:]+@[^\s/:]+:.+$/.test(value)) return 'scp';
  return 'local';
}

export function normalizeRepository(value, cwd = process.cwd()) {
  const kind = repositoryKind(value);
  if (kind === 'file') return fileURLToPath(value);
  return kind === 'local' ? path.resolve(cwd, value) : value;
}

export function resolveRepository(reference, specRepository) {
  const kind = repositoryKind(reference);
  const baseKind = repositoryKind(specRepository);
  const relative = reference.startsWith('./') || reference.startsWith('../');
  if (!relative) {
    if (['url', 'scp'].includes(baseKind) && ['local', 'file'].includes(kind)) {
      throw new Error('Een remote spec mag geen absoluut lokaal repositorypad gebruiken.');
    }
    return normalizeRepository(reference);
  }
  if (baseKind === 'url') return new URL(reference, specRepository.replace(/\/$/, '') + '/').href;
  if (baseKind === 'scp') {
    const colon = specRepository.indexOf(':');
    return specRepository.slice(0, colon + 1) + path.posix.normalize(path.posix.join(specRepository.slice(colon + 1), reference));
  }
  return path.resolve(normalizeRepository(specRepository), reference);
}

function objectKeys(value, expected, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} moet een object zijn.`);
  if (Object.keys(value).sort().join(',') !== [...expected].sort().join(',')) {
    throw new Error(`${label} vereist precies: ${expected.join(', ')}.`);
  }
}

async function readJson(filename) {
  const info = await fs.lstat(filename);
  if (!info.isFile() || info.size > 128 * 1024) throw new Error(`Ongeldig configuratiebestand: ${filename}`);
  return JSON.parse(await fs.readFile(filename, 'utf8'));
}

export async function readProject(specDirectory) {
  const manifest = await readJson(path.join(specDirectory, 'project.json'));
  const lock = await readJson(path.join(specDirectory, 'guidelines.lock.json'));
  objectKeys(manifest, ['schemaVersion', 'name', 'code', 'guidelines', 'profile'], 'project.json');
  objectKeys(manifest.code, ['repository'], 'code');
  objectKeys(manifest.guidelines, ['repository', 'ref'], 'guidelines');
  objectKeys(lock, ['schemaVersion', 'ref', 'commit'], 'guidelines.lock.json');
  if (manifest.schemaVersion !== 1 || lock.schemaVersion !== 1) throw new Error('Alleen schemaVersion 1 wordt ondersteund.');
  if (typeof manifest.name !== 'string' || !/^[A-Za-z][A-Za-z0-9._-]*$/.test(manifest.name)) throw new Error('Ongeldige projectnaam.');
  if (typeof manifest.profile !== 'string' || !/^[A-Za-z][A-Za-z0-9_-]*$/.test(manifest.profile)) throw new Error('Ongeldig profiel.');
  repositoryKind(manifest.code.repository);
  repositoryKind(manifest.guidelines.repository);
  if (typeof manifest.guidelines.ref !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(manifest.guidelines.ref)) throw new Error('Ongeldige richtlijnenref.');
  if (lock.ref !== manifest.guidelines.ref || typeof lock.commit !== 'string' || !/^([0-9a-f]{40}|[0-9a-f]{64})$/.test(lock.commit)) {
    throw new Error('De lock moet dezelfde ref en een volledige exacte commit bevatten.');
  }
  const specInfo = await fs.lstat(path.join(specDirectory, 'spec'));
  if (!specInfo.isDirectory()) throw new Error('De spec/-map ontbreekt of is geen gewone map.');
  return { manifest, lock };
}

async function exists(filename) {
  try { await fs.lstat(filename); return true; }
  catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}

async function createTarget(directory) {
  const target = path.resolve(directory);
  if (await exists(target)) throw new Error(`Doelmap bestaat al; niets overschreven: ${target}`);
  let parent = path.dirname(target);
  while (!(await exists(parent))) parent = path.dirname(parent);
  let gitRoot;
  try { gitRoot = git(['rev-parse', '--show-toplevel'], parent); } catch { /* A workspace has no enclosing Git repository. */ }
  if (gitRoot) throw new Error('Maak de werkruimte buiten bestaande Git-repositories; private context hoort niet in productcode.');
  await fs.mkdir(path.dirname(target), { recursive: true });
  await fs.mkdir(target);
  return target;
}

export function repositoryRoot(directory) {
  if (typeof directory !== 'string' || !directory) throw new Error('Een bewerkbare richtlijnenmap is nodig; gebruik cspec guidelines set <pad-naar-CretAI>.');
  const absolute = path.resolve(directory);
  const root = path.resolve(git(['rev-parse', '--show-toplevel'], absolute));
  const canonical = value => process.platform === 'win32' ? value.toLowerCase() : value;
  if (canonical(root) !== canonical(absolute)) throw new Error(`Gebruik de root van de repository: ${root}`);
  return root;
}

function originOrLocal(directory) {
  try { return normalizeRepository(git(['remote', 'get-url', 'origin'], directory), directory); }
  catch { return directory; }
}

async function writeJson(filename, value) {
  await fs.writeFile(filename, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
}

async function finishWorkspace(target, code, spec, specRepository, guidelinesRoot, progress) {
  const { manifest, lock } = await readProject(spec);
  const guidelinesRepository = resolveRepository(manifest.guidelines.repository, specRepository);
  const localDirectory = path.join(target, '.local');
  const activeGuidelines = path.join(localDirectory, 'guidelines');
  await fs.mkdir(localDirectory);
  progress('Vastgezette richtlijnen ophalen');
  git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', guidelinesRepository, activeGuidelines]);
  const referencedCommit = git(['rev-parse', '--verify', `${lock.ref}^{commit}`], activeGuidelines);
  if (referencedCommit !== lock.commit) throw new Error('De richtlijnentag wijkt af van de lock. Controleer de private spec; niets automatisch bijgewerkt.');
  git(['checkout', '--detach', lock.commit], activeGuidelines);
  const profilePath = path.join(activeGuidelines, 'profiles', `${manifest.profile}.md`);
  const profileInfo = await fs.lstat(profilePath);
  if (!profileInfo.isFile()) throw new Error('Het gekozen profiel is geen gewoon bestand.');
  const editableGuidelines = repositoryRoot(guidelinesRoot);
  try {
    if (git(['cat-file', '-t', lock.commit], editableGuidelines) !== 'commit') throw new Error('Geen commit.');
  } catch {
    throw new Error(`De ingestelde richtlijnenmap bevat commit ${lock.commit} niet.\nHaal de richtlijnen op met git fetch --tags in ${editableGuidelines}, of stel de juiste map in met cspec guidelines set.`);
  }
  const workspaceFile = path.join(target, 'project.code-workspace');
  await writeJson(workspaceFile, {
    folders: [
      { name: 'Code', path: path.relative(target, code).split(path.sep).join('/') },
      { name: 'Spec', path: path.relative(target, spec).split(path.sep).join('/') },
      { name: 'Algemene richtlijnen', path: editableGuidelines.split(path.sep).join('/') },
    ],
  });
  await fs.writeFile(path.join(localDirectory, 'context.md'), [
    `# ${manifest.name} — lokale werkcontext`, '',
    `Code: ${code}`, `Spec: ${spec}`, `Bewerkbare CretAI-bron: ${editableGuidelines}`, '',
    `Actieve richtlijnen: ${activeGuidelines}`, `Ref: ${lock.ref}`, `Commit: ${lock.commit}`, `Profiel: ${manifest.profile}`, '',
    'Dit overzicht laadt geen instructies of skills automatisch. Gebruik een expliciet ingerichte adapter.',
    'Werk nieuwe algemene afspraken uit in de bewerkbare bron; de actieve checkout is een vastgezette versie.', '',
  ].join('\n'), { flag: 'wx' });
  await writeJson(path.join(target, '.cretspec-workspace.json'), {
    schemaVersion: 1, name: manifest.name, code, spec, editableGuidelines, activeGuidelines,
    guidelinesCommit: lock.commit, workspaceFile,
  });
  return { target, workspaceFile, manifest, lock };
}

export async function cloneProject(specRepository, directory, { guidelinesRoot, progress = () => {} } = {}) {
  const source = normalizeRepository(specRepository);
  repositoryRoot(guidelinesRoot);
  const target = await createTarget(directory);
  try {
    const spec = path.join(target, 'spec');
    const code = path.join(target, 'code');
    progress('Private spec ophalen');
    git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', source, spec]);
    const { manifest } = await readProject(spec);
    progress('Code ophalen');
    git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', resolveRepository(manifest.code.repository, source), code]);
    return await finishWorkspace(target, code, spec, source, guidelinesRoot, progress);
  } catch (error) {
    throw new Error(`${error.message}\nOnvoltooide nieuwe werkmap bewaard: ${target}\nBronrepositories zijn niet gewijzigd.`);
  }
}

export async function attachProject(codeDirectory, specDirectory, directory, { guidelinesRoot, progress = () => {} } = {}) {
  const code = repositoryRoot(codeDirectory);
  const spec = repositoryRoot(specDirectory);
  repositoryRoot(guidelinesRoot);
  await readProject(spec);
  const target = await createTarget(directory);
  try { return await finishWorkspace(target, code, spec, originOrLocal(spec), guidelinesRoot, progress); }
  catch (error) { throw new Error(`${error.message}\nOnvoltooide nieuwe werkmap bewaard: ${target}\nBestaande code en spec zijn niet gewijzigd.`); }
}

export async function workspacePath(start = process.cwd()) {
  let current = path.resolve(start);
  if ((await fs.stat(current)).isFile()) current = path.dirname(current);
  while (true) {
    const marker = path.join(current, '.cretspec-workspace.json');
    if (await exists(marker)) {
      const data = await readJson(marker);
      const expected = path.join(current, 'project.code-workspace');
      if (data.schemaVersion !== 1 || data.workspaceFile !== expected) throw new Error('Ongeldige lokale workspaceadministratie.');
      await fs.access(expected);
      return expected;
    }
    const parent = path.dirname(current);
    if (parent === current) throw new Error('Geen CretSpec-werkruimte gevonden. Geef het pad van de omvattende werkmap mee.');
    current = parent;
  }
}

export async function openPath(filename) {
  const absolute = path.resolve(filename);
  await fs.access(absolute);
  const command = process.platform === 'win32' ? 'explorer.exe' : process.platform === 'darwin' ? 'open' : 'xdg-open';
  await new Promise((resolve, reject) => {
    const child = spawn(command, [absolute], { shell: false, windowsHide: true, stdio: 'ignore', detached: true });
    child.once('error', () => reject(new Error(`Open het pad handmatig; ${command} kon niet worden gestart: ${absolute}`)));
    child.once('spawn', () => { child.unref(); resolve(); });
  });
}
