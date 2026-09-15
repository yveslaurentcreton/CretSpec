import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { git, normalizeNamespace, normalizeRepository, resolveRepository, resolveSpecSource, cloneProject, attachProject, workspacePath, projectInfo, findSpec } from '../src/workspace.mjs';

const cli = fileURLToPath(new URL('../bin/cspec.mjs', import.meta.url));

async function temporary(t) {
  const base = await fs.realpath(os.tmpdir());
  const root = await fs.mkdtemp(path.join(base, 'cretspec-test-'));
  t.after(async () => {
    const resolved = await fs.realpath(root);
    const relative = path.relative(base, resolved);
    assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative));
    assert.ok(path.basename(resolved).startsWith('cretspec-test-'));
    await fs.rm(resolved, { recursive: true, force: true, maxRetries: 4, retryDelay: 100 });
  });
  return root;
}

function commit(directory, message) {
  git(['add', '.'], directory);
  git(['-c', 'user.name=Test Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-m', message], directory);
  return git(['rev-parse', 'HEAD'], directory);
}

async function repository(directory, files) {
  await fs.mkdir(directory, { recursive: true });
  git(['init', '-b', 'main'], directory);
  for (const [name, content] of Object.entries(files)) {
    const filename = path.join(directory, name);
    await fs.mkdir(path.dirname(filename), { recursive: true });
    await fs.writeFile(filename, content);
  }
  return commit(directory, 'Fixture');
}

async function fixture(t) {
  const root = await temporary(t);
  const source = path.join(root, 'source repositories');
  const guidelines = path.join(source, 'CretAI');
  const code = path.join(source, 'Sample');
  const spec = path.join(source, 'Sample-spec');
  const hash = await repository(guidelines, { 'guidelines/principles.md': '# Rules\n', 'profiles/dotnet.md': '# .NET\n' });
  git(['tag', 'v0.1.0'], guidelines);
  await repository(code, { 'README.md': '# Code\n' });
  const manifest = {
    schemaVersion: 1, name: 'Sample', code: { repository: '../Sample' },
    guidelines: { repository: '../CretAI', ref: 'v0.1.0' }, profile: 'dotnet',
  };
  const lock = { schemaVersion: 1, ref: 'v0.1.0', commit: hash };
  await repository(spec, {
    'project.json': JSON.stringify(manifest), 'guidelines.lock.json': JSON.stringify(lock),
    'spec/vision.md': '# Spec\n',
  });
  const destination = path.join(root, 'destination with spaces');
  await fs.mkdir(destination);
  return { root, source, destination, guidelines, code, spec, manifest, lock, hash };
}

function runner(f, executable = cli) {
  return (...args) => execFileSync(process.execPath, [executable, ...args], {
    cwd: f.destination, encoding: 'utf8', stdio: 'pipe',
    env: { ...process.env, CRETSPEC_HOME: path.join(f.root, 'personal config') },
  }).trim();
}

test('repository references follow their source location across transports', () => {
  assert.equal(resolveRepository('../Code', 'https://github.com/owner/Code-spec.git'), 'https://github.com/owner/Code');
  assert.equal(resolveRepository('../CretAI.git', 'ssh://git@example.test:2222/group/Code-spec.git'), 'ssh://git@example.test:2222/group/CretAI.git');
  assert.equal(resolveRepository('../Code', 'git@example.test:group/Code-spec.git'), 'git@example.test:group/Code');
  const local = path.resolve('source with spaces', 'Code-spec');
  assert.equal(resolveRepository('../Code', local), path.resolve('source with spaces', 'Code'));
  assert.equal(normalizeRepository(pathToFileURL(local).href), local);
  assert.throws(() => normalizeRepository('ext::untrusted command'), /helpers/);
  assert.throws(() => normalizeRepository('https://token@example.test/repo'), /credentials/);
  assert.throws(() => resolveRepository(path.resolve('private-local'), 'https://example.test/spec'), /absolute local/);
});

test('short names use the repository namespace, while explicit paths stay local', async t => {
  const f = await fixture(t);
  assert.equal(normalizeNamespace('owner'), 'git@github.com:owner');
  assert.equal(resolveSpecSource('Sample-spec', f.source), f.spec);
  assert.equal(resolveSpecSource('Sample-spec', 'git@github.com:owner'), 'git@github.com:owner/Sample-spec');
  assert.equal(resolveSpecSource('Sample-spec', 'https://github.com/owner/'), 'https://github.com/owner/Sample-spec');
  assert.equal(resolveSpecSource('./Sample-spec', f.source, f.destination), path.join(f.destination, 'Sample-spec'));
  assert.equal(resolveSpecSource(f.spec), f.spec);
});

test('one-argument clone creates one project root with three repositories and no outer definition', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.guidelines, 'uncommitted.md'), 'Private draft');
  const info = await cloneProject('Sample-spec', undefined, { repositoryNamespace: f.source, cwd: f.destination });
  assert.equal(info.projectRoot, path.join(f.destination, 'Sample'));
  assert.equal(info.spec, path.join(info.projectRoot, 'Sample-spec'));
  assert.equal(info.code, path.join(info.projectRoot, 'Sample'));
  assert.equal(info.editableGuidelines, path.join(info.projectRoot, 'CretAI'));
  assert.deepEqual(await fs.readdir(f.destination), ['Sample']);
  assert.deepEqual((await fs.readdir(info.projectRoot)).sort(), ['CretAI', 'Sample', 'Sample-spec']);
  assert.equal(git(['rev-parse', '--abbrev-ref', 'HEAD'], info.editableGuidelines), 'main');
  assert.equal(git(['ls-files'], info.code), 'README.md');
  assert.equal(git(['status', '--porcelain'], info.code), '');
  assert.equal(git(['status', '--porcelain'], info.spec), '');
  assert.equal(git(['rev-parse', 'HEAD'], info.activeGuidelines), f.hash);
  assert.equal(git(['rev-parse', '--abbrev-ref', 'HEAD'], info.activeGuidelines), 'HEAD');
  await assert.rejects(fs.access(path.join(info.activeGuidelines, 'uncommitted.md')));
  await assert.rejects(fs.access(path.join(info.spec, '.local', 'project.code-workspace')));
  assert.match(git(['status', '--porcelain'], f.guidelines), /uncommitted.md/);
});

test('editor files are disposable, stay in the spec and read the current manifest', async t => {
  const f = await fixture(t);
  const options = { cwd: f.destination };
  const info = await cloneProject(f.spec, undefined, options);
  const filename = await workspacePath(path.join(info.spec, 'spec'), options);
  assert.equal(filename, path.join(info.spec, '.local', 'project.code-workspace'));
  const workspace = JSON.parse(await fs.readFile(filename, 'utf8'));
  assert.equal(path.resolve(path.dirname(filename), workspace.folders[0].path), info.code);
  assert.equal(path.resolve(path.dirname(filename), workspace.folders[2].path), info.editableGuidelines);
  workspace.settings = { 'editor.tabSize': 2 };
  workspace.folders[0].path = 'stale-editor-path';
  await fs.writeFile(filename, JSON.stringify(workspace));
  const newCode = path.join(info.projectRoot, 'RenamedSample');
  await fs.rename(info.code, newCode);
  await fs.writeFile(path.join(info.spec, 'project.json'), JSON.stringify({ ...f.manifest, name: 'RenamedSample' }));
  await workspacePath(info.spec, options);
  const regenerated = JSON.parse(await fs.readFile(filename, 'utf8'));
  assert.equal(path.resolve(path.dirname(filename), regenerated.folders[0].path), newCode);
  assert.equal(regenerated.settings['editor.tabSize'], 2);
  await fs.unlink(filename);
  assert.equal(await workspacePath(info.spec, options), filename);
  assert.deepEqual((await fs.readdir(info.projectRoot)).sort(), ['CretAI', 'RenamedSample', 'Sample-spec']);
});

test('moving a whole project needs no registry, source connection or rewritten definition', async t => {
  const f = await fixture(t);
  const info = await cloneProject(f.spec, undefined, { cwd: f.destination });
  await workspacePath(info.spec, {});
  const relocated = path.join(f.root, 'relocated project');
  await fs.rename(info.projectRoot, relocated);
  await fs.rename(f.guidelines, path.join(f.root, 'unavailable original guidelines'));
  const spec = path.join(relocated, 'Sample-spec');
  const current = await projectInfo(relocated);
  assert.equal(current.code, path.join(relocated, 'Sample'));
  const filename = await workspacePath(spec, {});
  const workspace = JSON.parse(await fs.readFile(filename, 'utf8'));
  assert.equal(path.resolve(path.dirname(filename), workspace.folders[2].path), path.join(relocated, 'CretAI'));
  assert.equal(git(['status', '--porcelain'], spec), '');
});

test('existing code, spec destinations and enclosing repositories are protected', async t => {
  const f = await fixture(t);
  const options = { cwd: f.destination };
  const target = path.join(f.destination, 'existing');
  await fs.mkdir(target);
  await fs.writeFile(path.join(target, 'keep.txt'), 'Keep me');
  await assert.rejects(cloneProject(f.spec, target, options), /already exists/);
  assert.equal(await fs.readFile(path.join(target, 'keep.txt'), 'utf8'), 'Keep me');
  await assert.rejects(cloneProject(f.spec, path.join(f.code, 'private-spec'), options), /outside existing Git/);
  const code = path.join(f.destination, 'Sample');
  await fs.mkdir(code);
  await fs.writeFile(path.join(code, 'keep.txt'), 'Keep code');
  await assert.rejects(cloneProject(f.spec, undefined, options), /already exists/);
  assert.equal(await fs.readFile(path.join(code, 'keep.txt'), 'utf8'), 'Keep code');
});

test('attach preserves uncommitted work and rejects a second source of path bindings', async t => {
  const f = await fixture(t);
  const spec = path.join(f.destination, 'Sample-spec');
  const code = path.join(f.destination, 'Sample');
  git(['clone', '--', f.spec, spec]);
  git(['clone', '--', f.code, code]);
  await fs.writeFile(path.join(code, 'README.md'), '# User changes\n');
  const info = await attachProject(code, spec, {});
  assert.equal(info.spec, spec);
  assert.equal(await fs.readFile(path.join(code, 'README.md'), 'utf8'), '# User changes\n');
  assert.equal(git(['status', '--porcelain'], spec), '');
  await assert.rejects(attachProject(f.code, spec, {}), /sibling directory/);
  assert.deepEqual((await fs.readdir(f.destination)).sort(), ['CretAI', 'Sample', 'Sample-spec']);
});

test('a moved guidelines tag or changed cache cannot silently replace locked rules', async t => {
  const f = await fixture(t);
  const options = { cwd: f.destination };
  const info = await cloneProject(f.spec, undefined, options);
  await fs.writeFile(path.join(info.activeGuidelines, 'guidelines', 'principles.md'), '# Changed cache\n');
  await assert.rejects(projectInfo(info.spec, options), /cached guidelines/);
  await fs.writeFile(path.join(f.guidelines, 'new.md'), '# New rules\n');
  commit(f.guidelines, 'Changed rules');
  git(['tag', '-f', 'v0.1.0'], f.guidelines);
  await assert.rejects(cloneProject(f.spec, path.join(f.root, 'new attempt', 'Sample-spec'), options), /does not match the lock/);
});

test('invalid or executable manifest fields fail before any code clone', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.spec, 'project.json'), JSON.stringify({ ...f.manifest, run: 'untrusted command' }));
  commit(f.spec, 'Invalid manifest fixture');
  await assert.rejects(cloneProject(f.spec, undefined, { cwd: f.destination }), /requires exactly/);
  await assert.rejects(fs.access(path.join(f.destination, 'Sample')));
  assert.equal(git(['status', '--porcelain'], f.code), '');
});

test('English help and version need no configuration; setup errors explain the next step', async t => {
  const f = await fixture(t);
  const run = runner(f);
  assert.match(run('--help'), /Usage:/);
  assert.match(run('--help'), /cspec project clone CretQL-spec/);
  const pkg = JSON.parse(await fs.readFile(new URL('../package.json', import.meta.url), 'utf8'));
  assert.equal(run('--version'), pkg.version);
  assert.throws(() => run('project', 'clone', 'Sample-spec'), error => {
    assert.match(String(error.stderr), /Configure.*cspec config namespace/);
    return true;
  });
  assert.throws(() => run('project', 'open', '--unsupported'));
});

test('a packaged CLI outside Git supports the short clone command and spec discovery', async t => {
  const f = await fixture(t);
  const installed = path.join(f.root, 'installed tool');
  const source = fileURLToPath(new URL('../', import.meta.url));
  for (const name of ['bin', 'src', 'package.json']) {
    await fs.cp(path.join(source, name), path.join(installed, name), { recursive: true });
  }
  const run = runner(f, path.join(installed, 'bin', 'cspec.mjs'));
  run('config', 'namespace', f.source);
  run('project', 'clone', 'Sample-spec');
  const root = path.join(f.destination, 'Sample');
  const spec = path.join(root, 'Sample-spec');
  await fs.unlink(path.join(f.root, 'personal config', 'config.json'));
  const info = JSON.parse(run('project', 'info', root));
  assert.equal(info.editableGuidelines, path.join(root, 'CretAI'));
  assert.equal(info.code, path.join(root, 'Sample'));
  assert.equal(run('guidelines', 'path', root), info.editableGuidelines);
  assert.equal(run('guidelines', 'edit', info.code, '--print'), info.editableGuidelines);
  assert.equal(JSON.parse(run('project', 'info', info.code)).spec, spec);
  assert.equal(run('project', 'open', spec, '--print'), path.join(spec, '.local', 'project.code-workspace'));
  await assert.rejects(fs.access(path.join(installed, '.git')));
  assert.deepEqual((await fs.readdir(root)).sort(), ['CretAI', 'Sample', 'Sample-spec']);
});

test('namespace settings preserve preferences and replace legacy paths even after relocation', async t => {
  const f = await fixture(t);
  const run = runner(f);
  run('config', 'namespace', 'owner');
  const configFile = path.join(f.root, 'personal config', 'config.json');
  const before = await fs.readFile(configFile, 'utf8');
  assert.throws(() => run('config', 'namespace', 'https://token@example.test/owner'));
  assert.equal(await fs.readFile(configFile, 'utf8'), before);
  await fs.writeFile(configFile, JSON.stringify({ schemaVersion: 1, guidelinesRoot: f.guidelines, preference: 'keep' }));
  assert.equal(run('config', 'namespace'), f.source);
  await fs.rename(f.guidelines, path.join(f.root, 'relocated guidelines'));
  run('config', 'namespace', 'owner');
  assert.deepEqual(JSON.parse(await fs.readFile(configFile, 'utf8')), {
    schemaVersion: 2, repositoryNamespace: 'git@github.com:owner', preference: 'keep',
  });
});

test('unrelated guidelines and tracked local files cannot substitute private context', async t => {
  const f = await fixture(t);
  const unrelated = path.join(f.root, 'unrelated rules');
  await repository(unrelated, { 'guidelines/principles.md': '# Other rules\n', 'profiles/dotnet.md': '# Other profile\n' });
  const info = await cloneProject(f.spec, undefined, { cwd: f.destination });
  git(['remote', 'set-url', 'origin', unrelated], info.editableGuidelines);
  await assert.rejects(projectInfo(info.projectRoot), /does not match guidelines.repository/);
  await fs.mkdir(path.join(f.spec, '.local'));
  await fs.writeFile(path.join(f.spec, '.local', 'keep.txt'), 'Tracked file');
  commit(f.spec, 'Tracked local file fixture');
  await assert.rejects(attachProject(f.code, f.spec, {}), /tracks .local/);
  assert.equal(await fs.readFile(path.join(f.spec, '.local', 'keep.txt'), 'utf8'), 'Tracked file');
});

test('a spec rejects unrelated existing code at the expected path without changing it', async t => {
  const f = await fixture(t);
  const spec = path.join(f.destination, 'Sample-spec');
  const code = path.join(f.destination, 'Sample');
  git(['clone', '--', f.spec, spec]);
  const previous = await repository(code, { 'README.md': '# Old independent product\n' });
  await assert.rejects(projectInfo(spec, {}), /does not match code.repository/);
  assert.equal(git(['rev-parse', 'HEAD'], code), previous);
  assert.equal(git(['status', '--porcelain'], code), '');
  assert.equal(git(['status', '--porcelain'], spec), '');
  await assert.rejects(fs.access(path.join(spec, '.local')));
});

test('each project has its own editable guidelines clone while active rules remain pinned', async t => {
  const f = await fixture(t);
  const first = await cloneProject(f.spec, 'First project', { cwd: f.destination });
  const second = await cloneProject(f.spec, 'Second project', { cwd: f.destination });
  await fs.writeFile(path.join(first.editableGuidelines, 'guidelines/principles.md'), '# Local proposal\n');
  assert.equal((await fs.readFile(path.join(second.editableGuidelines, 'guidelines/principles.md'), 'utf8')).trim(), '# Rules');
  assert.equal(git(['remote', 'get-url', 'origin'], first.editableGuidelines), git(['remote', 'get-url', 'origin'], second.editableGuidelines));
  assert.equal((await projectInfo(first.code)).editableGuidelines, first.editableGuidelines);
  assert.equal((await projectInfo(second.spec)).editableGuidelines, second.editableGuidelines);
  assert.equal((await fs.readFile(path.join(first.activeGuidelines, 'guidelines/principles.md'), 'utf8')).trim(), '# Rules');
  assert.equal(git(['rev-parse', 'HEAD'], second.activeGuidelines), f.hash);
});

test('project discovery rejects ambiguous roots and accepts an explicit spec', async t => {
  const f = await fixture(t);
  const info = await cloneProject(f.spec, undefined, { cwd: f.destination });
  git(['clone', '--', f.spec, path.join(info.projectRoot, 'Another-spec')]);
  await assert.rejects(findSpec(info.projectRoot), /Multiple project specs/);
  assert.equal(await findSpec(info.spec), info.spec);
});
