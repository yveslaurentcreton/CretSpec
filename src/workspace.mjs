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
    if (error.code === 'ENOENT') throw new Error('Git was not found. Install Git and check your PATH.');
    const detail = String(error.stderr || error.message).replace(/(https?:\/\/)[^\s/@]+@/g, '$1[redacted]@');
    throw new Error(`Git command failed: ${detail.trim()}`);
  }
}

function repositoryKind(value) {
  if (typeof value !== 'string' || !value.trim() || /[\r\n\0]/.test(value)) {
    throw new Error('A repository reference must be a non-empty URL or path.');
  }
  if (/^[A-Za-z][A-Za-z0-9+.-]*::/.test(value)) throw new Error('Git remote helpers are not allowed.');
  if (/^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(value)) {
    const url = new URL(value);
    if (!['https:', 'ssh:', 'file:'].includes(url.protocol)) throw new Error('Use HTTPS, SSH or a local Git path.');
    if (url.password || (url.protocol === 'https:' && url.username) || url.search || url.hash) {
      throw new Error('Do not put credentials, query parameters or fragments in repository URLs.');
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
      throw new Error('A remote spec cannot reference an absolute local repository path.');
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

const samePath = (a, b) => {
  const normalize = value => process.platform === 'win32' ? path.resolve(value).toLowerCase() : path.resolve(value);
  return normalize(a) === normalize(b);
};

function portableName(value) {
  return typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value)
    && !value.endsWith('.') && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value);
}

export function repositoryRoot(directory) {
  if (typeof directory !== 'string' || !directory) throw new Error('A repository directory is required.');
  const absolute = path.resolve(directory);
  const root = path.resolve(git(['rev-parse', '--show-toplevel'], absolute));
  if (!samePath(root, absolute)) throw new Error(`Use the repository root: ${root}`);
  return root;
}

export function originOrLocal(directory) {
  try { return normalizeRepository(git(['remote', 'get-url', 'origin'], directory), directory); }
  catch { return directory; }
}

export function normalizeNamespace(value, cwd = process.cwd()) {
  if (portableName(value)) return `git@github.com:${value}`;
  const normalized = normalizeRepository(value, cwd);
  return repositoryKind(normalized) === 'local' ? normalized : normalized.replace(/\/+$/, '');
}

export function namespaceFromRepository(repository) {
  return resolveRepository('../', normalizeRepository(repository));
}

export function resolveSpecSource(input, repositoryNamespace, cwd = process.cwd()) {
  if (portableName(input)) {
    if (!repositoryNamespace) throw new Error('Configure a repository namespace first: cspec config namespace <GitHub-owner-or-namespace-URL>');
    const base = normalizeNamespace(repositoryNamespace, cwd);
    return repositoryKind(base) === 'local' ? path.join(base, input) : base + '/' + input;
  }
  return normalizeRepository(input, cwd);
}

function repositoryDirectory(source) {
  const kind = repositoryKind(source);
  const value = kind === 'url' ? new URL(source).pathname : kind === 'scp' ? source.slice(source.indexOf(':') + 1) : source;
  const name = (kind === 'local' ? path.basename(value) : path.posix.basename(value.replace(/\/$/, ''))).replace(/\.git$/, '');
  if (!portableName(name)) throw new Error('This repository needs an explicit destination directory with a portable name.');
  return name;
}

function objectKeys(value, expected, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} must be an object.`);
  if (Object.keys(value).sort().join(',') !== [...expected].sort().join(',')) {
    throw new Error(`${label} requires exactly: ${expected.join(', ')}.`);
  }
}

async function readJson(filename) {
  const info = await fs.lstat(filename);
  if (!info.isFile() || info.size > 128 * 1024) throw new Error(`Invalid configuration file: ${filename}`);
  return JSON.parse(await fs.readFile(filename, 'utf8'));
}

export async function readProject(specDirectory) {
  const manifest = await readJson(path.join(specDirectory, 'project.json'));
  const lock = await readJson(path.join(specDirectory, 'guidelines.lock.json'));
  objectKeys(manifest, ['schemaVersion', 'name', 'code', 'guidelines', 'profile'], 'project.json');
  objectKeys(manifest.code, ['repository'], 'code');
  objectKeys(manifest.guidelines, ['repository', 'ref'], 'guidelines');
  objectKeys(lock, ['schemaVersion', 'ref', 'commit'], 'guidelines.lock.json');
  if (manifest.schemaVersion !== 1 || lock.schemaVersion !== 1) throw new Error('Only schemaVersion 1 is supported.');
  if (!portableName(manifest.name) || !/^[A-Za-z]/.test(manifest.name)) throw new Error('Invalid or non-portable project name.');
  if (typeof manifest.profile !== 'string' || !/^[A-Za-z][A-Za-z0-9_-]*$/.test(manifest.profile)) throw new Error('Invalid profile.');
  repositoryKind(manifest.code.repository);
  repositoryKind(manifest.guidelines.repository);
  if (typeof manifest.guidelines.ref !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(manifest.guidelines.ref)) throw new Error('Invalid guidelines ref.');
  if (lock.ref !== manifest.guidelines.ref || typeof lock.commit !== 'string' || !/^([0-9a-f]{40}|[0-9a-f]{64})$/.test(lock.commit)) {
    throw new Error('The lock must contain the same ref and a full exact commit ID.');
  }
  const specInfo = await fs.lstat(path.join(specDirectory, 'spec'));
  if (!specInfo.isDirectory()) throw new Error('The spec/ directory is missing or is not a regular directory.');
  return { manifest, lock };
}

async function exists(filename) {
  try { await fs.lstat(filename); return true; }
  catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}

async function prepareParent(directory) {
  const target = path.resolve(directory);
  let parent = target;
  while (!(await exists(parent))) parent = path.dirname(parent);
  let gitRoot;
  try { gitRoot = git(['rev-parse', '--show-toplevel'], parent); } catch { /* Standalone clones have no enclosing Git repository. */ }
  if (gitRoot) throw new Error('Create the project outside existing Git repositories.');
  await fs.mkdir(target, { recursive: true });
  return target;
}

async function createTarget(directory) {
  const target = path.resolve(directory);
  if (await exists(target)) throw new Error(`Destination already exists; nothing overwritten: ${target}`);
  await prepareParent(path.dirname(target));
  await fs.mkdir(target);
  return target;
}

function codePath(spec, manifest) {
  const code = path.join(path.dirname(spec), manifest.name);
  if (samePath(code, spec)) throw new Error('The spec directory and the sibling code directory must have different names.');
  return code;
}

function projectPaths(spec, manifest, source = originOrLocal(spec)) {
  const projectRoot = path.dirname(spec);
  const code = codePath(spec, manifest);
  const guidelinesRepository = resolveRepository(manifest.guidelines.repository, source);
  const editableGuidelines = path.join(projectRoot, repositoryDirectory(guidelinesRepository));
  const names = [spec, code, editableGuidelines].map(directory => path.basename(directory).toLowerCase());
  if (new Set(names).size !== names.length) throw new Error('Spec, code and guidelines need distinct directory names inside the project root.');
  return { projectRoot, code, editableGuidelines, guidelinesRepository };
}

function repositoryIdentity(reference) {
  const source = normalizeRepository(reference);
  const kind = repositoryKind(source);
  if (kind === 'url') {
    const url = new URL(source);
    const port = url.port && !['22', '443'].includes(url.port) ? ':' + url.port : '';
    return url.hostname.toLowerCase() + port + url.pathname.replace(/\/$/, '').replace(/\.git$/, '');
  }
  if (kind === 'scp') {
    const hostStart = source.indexOf('@') + 1;
    const colon = source.indexOf(':', hostStart);
    return source.slice(hostStart, colon).toLowerCase() + '/' + source.slice(colon + 1).replace(/\/$/, '').replace(/\.git$/, '');
  }
  return process.platform === 'win32' ? path.resolve(source).toLowerCase() : path.resolve(source);
}

async function localDirectory(spec) {
  if (git(['ls-files', '--', '.local'], spec)) throw new Error('The spec tracks .local/ files. Untrack those files before generating local context.');
  const local = path.join(spec, '.local');
  if (await exists(local)) {
    if (!(await fs.lstat(local)).isDirectory()) throw new Error('The spec .local/ path must be a regular directory, not a link.');
  } else {
    await fs.mkdir(local);
  }
  try { git(['check-ignore', '--quiet', '--', '.local/cspec-probe'], spec); }
  catch {
    const exclude = path.resolve(spec, git(['rev-parse', '--git-path', 'info/exclude'], spec));
    await fs.mkdir(path.dirname(exclude), { recursive: true });
    await fs.appendFile(exclude, '\n# CretSpec generated local files\n/.local/\n');
  }
  return local;
}

async function guidelinesSnapshot(spec, manifest, lock, progress) {
  const { editableGuidelines, guidelinesRepository } = projectPaths(spec, manifest);
  if (!(await exists(editableGuidelines))) {
    progress('Fetching project guidelines');
    git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', guidelinesRepository, editableGuidelines]);
  }
  if (!(await fs.lstat(editableGuidelines)).isDirectory()) throw new Error('The project guidelines must be a regular directory.');
  repositoryRoot(editableGuidelines);
  if (repositoryIdentity(originOrLocal(editableGuidelines)) !== repositoryIdentity(guidelinesRepository)) {
    throw new Error('The project guidelines repository does not match guidelines.repository in the spec.');
  }
  try {
    if (git(['cat-file', '-t', lock.commit], editableGuidelines) !== 'commit') throw new Error('Not a commit.');
  } catch {
    throw new Error(`The project guidelines repository does not contain ${lock.commit}.\nRun git fetch --tags in ${editableGuidelines}.`);
  }
  if (git(['rev-parse', '--verify', `${lock.ref}^{commit}`], editableGuidelines) !== lock.commit) {
    throw new Error('The guidelines ref does not match the lock. No version was adopted.');
  }
  const local = await localDirectory(spec);
  const cache = path.join(local, 'guidelines');
  if (await exists(cache)) {
    if (!(await fs.lstat(cache)).isDirectory()) throw new Error('The guidelines cache must be a regular directory.');
  } else {
    await fs.mkdir(cache);
  }
  const activeGuidelines = path.join(cache, lock.commit);
  if (!(await exists(activeGuidelines))) {
    progress('Fetching pinned guidelines');
    git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', editableGuidelines, activeGuidelines]);
    const referencedCommit = git(['rev-parse', '--verify', `${lock.ref}^{commit}`], activeGuidelines);
    if (referencedCommit !== lock.commit) throw new Error('The guidelines ref does not match the lock. No version was adopted.');
    git(['checkout', '--detach', lock.commit], activeGuidelines);
  }
  if (!(await fs.lstat(activeGuidelines)).isDirectory()) throw new Error('The guidelines snapshot must be a regular directory.');
  repositoryRoot(activeGuidelines);
  if (git(['rev-parse', 'HEAD'], activeGuidelines) !== lock.commit
    || git(['rev-parse', '--verify', `${lock.ref}^{commit}`], activeGuidelines) !== lock.commit
    || git(['status', '--porcelain', '--untracked-files=all'], activeGuidelines)) {
    throw new Error('The cached guidelines differ from the lock or contain local changes. Preserve any work and remove that snapshot before retrying.');
  }
  const profile = await fs.lstat(path.join(activeGuidelines, 'profiles', `${manifest.profile}.md`));
  if (!profile.isFile()) throw new Error('The selected profile must be a regular file.');
  return { editableGuidelines, activeGuidelines };
}

export async function findSpec(start = process.cwd()) {
  let current = path.resolve(start);
  if ((await fs.stat(current)).isFile()) current = path.dirname(current);
  while (true) {
    if (await exists(path.join(current, 'project.json')) && await exists(path.join(current, 'guidelines.lock.json'))) {
      const spec = repositoryRoot(current);
      await readProject(spec);
      return spec;
    }
    let enclosingRepository;
    try { enclosingRepository = path.resolve(git(['rev-parse', '--show-toplevel'], current)); } catch { /* An ordinary project container is not a Git repository. */ }
    if (enclosingRepository && !samePath(current, enclosingRepository)) {
      current = enclosingRepository;
      continue;
    }
    if (!enclosingRepository) {
      const candidates = [];
      for (const entry of await fs.readdir(current, { withFileTypes: true })) {
        if (!entry.isDirectory() || entry.name.startsWith('.')) continue;
        const directory = path.join(current, entry.name);
        if (await exists(path.join(directory, 'project.json')) && await exists(path.join(directory, 'guidelines.lock.json'))) candidates.push(directory);
      }
      if (candidates.length > 1) throw new Error('Multiple project specs found. Pass the intended spec directory explicitly.');
      if (candidates.length === 1) {
        const spec = repositoryRoot(candidates[0]);
        await readProject(spec);
        return spec;
      }
    }
    const parent = path.dirname(current);
    if (parent === current) throw new Error('No project spec found. Run this command inside a project root, code or spec repository, or pass a project directory.');
    current = parent;
  }
}

export async function projectInfo(start, { progress = () => {} } = {}) {
  const spec = await findSpec(start);
  const { manifest, lock } = await readProject(spec);
  const { projectRoot, code: codeDirectory } = projectPaths(spec, manifest);
  const code = repositoryRoot(codeDirectory);
  const expectedSource = resolveRepository(manifest.code.repository, originOrLocal(spec));
  if (repositoryIdentity(originOrLocal(code)) !== repositoryIdentity(expectedSource)) {
    throw new Error(`The sibling code repository does not match code.repository in the spec: ${code}\nExpected source: ${expectedSource}\nUse the correct clone; the existing repository has not been changed.`);
  }
  const guidelines = await guidelinesSnapshot(spec, manifest, lock, progress);
  return { projectRoot, spec, code, ...guidelines, manifest, lock };
}

export async function cloneProject(input, directory, { repositoryNamespace, cwd = process.cwd(), progress = () => {} } = {}) {
  const source = resolveSpecSource(input, repositoryNamespace, cwd);
  const explicitRoot = directory ? path.resolve(cwd, directory) : null;
  if (explicitRoot && await exists(explicitRoot)) throw new Error(`Destination already exists; nothing overwritten: ${explicitRoot}`);
  const parent = await prepareParent(explicitRoot ? path.dirname(explicitRoot) : cwd);
  const staging = await fs.mkdtemp(path.join(parent, '.cspec-clone-'));
  let projectRoot;
  let spec;
  try {
    progress('Fetching project spec');
    git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', source, staging]);
    const { manifest } = await readProject(staging);
    const target = explicitRoot || path.join(parent, manifest.name);
    const targetSpec = path.join(target, repositoryDirectory(source));
    const { code } = projectPaths(targetSpec, manifest, source);
    projectRoot = await createTarget(target);
    await fs.rename(staging, targetSpec);
    spec = targetSpec;
    progress('Fetching code');
    git(['clone', '--no-recurse-submodules', '--no-hardlinks', '--', resolveRepository(manifest.code.repository, source), code]);
    return await projectInfo(spec, { progress });
  } catch (error) {
    const preserved = [projectRoot, !spec ? staging : null].filter(Boolean).join(', ');
    throw new Error(`${error.message}\nNew directories preserved for inspection: ${preserved}\nExisting repositories have not been modified.`);
  }
}

export async function attachProject(codeDirectory, specDirectory, { progress = () => {} } = {}) {
  const spec = repositoryRoot(specDirectory);
  const code = repositoryRoot(codeDirectory);
  const { manifest } = await readProject(spec);
  if (!samePath(code, codePath(spec, manifest))) {
    throw new Error(`The spec expects its code in the sibling directory ${codePath(spec, manifest)}. No separate project bindings are stored.`);
  }
  return projectInfo(spec, { progress });
}

export async function workspacePath(start, options = {}) {
  const info = await projectInfo(start, options);
  const local = await localDirectory(info.spec);
  const workspaceFile = path.join(local, 'project.code-workspace');
  let existing = {};
  if (await exists(workspaceFile)) existing = await readJson(workspaceFile);
  const relative = directory => path.relative(local, directory).split(path.sep).join('/');
  await fs.writeFile(workspaceFile, JSON.stringify({
    ...existing,
    folders: [
      { name: path.basename(info.code), path: relative(info.code) },
      { name: path.basename(info.spec), path: relative(info.spec) },
      { name: path.basename(info.editableGuidelines), path: relative(info.editableGuidelines) },
    ],
  }, null, 2) + '\n');
  return workspaceFile;
}

export async function openPath(filename) {
  const absolute = path.resolve(filename);
  await fs.access(absolute);
  const command = process.platform === 'win32' ? 'explorer.exe' : process.platform === 'darwin' ? 'open' : 'xdg-open';
  await new Promise((resolve, reject) => {
    const child = spawn(command, [absolute], { shell: false, windowsHide: true, stdio: 'ignore', detached: true });
    child.once('error', () => reject(new Error(`Open the path manually; ${command} could not be started: ${absolute}`)));
    child.once('spawn', () => { child.unref(); resolve(); });
  });
}
