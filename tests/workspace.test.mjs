import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { git, normalizeRepository, resolveRepository, cloneProject, attachProject, workspacePath } from '../src/workspace.mjs';

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

async function repository(directory, files) {
  await fs.mkdir(directory, { recursive: true });
  git(['init', '-b', 'main'], directory);
  for (const [name, content] of Object.entries(files)) {
    const filename = path.join(directory, name);
    await fs.mkdir(path.dirname(filename), { recursive: true });
    await fs.writeFile(filename, content);
  }
  git(['add', '.'], directory);
  git(['-c', 'user.name=Test Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-m', 'Fixture'], directory);
  return git(['rev-parse', 'HEAD'], directory);
}

async function fixture(t) {
  const root = await temporary(t);
  const source = path.join(root, 'source repositories');
  const guidelines = path.join(source, 'CretAI');
  const code = path.join(source, 'Sample');
  const spec = path.join(source, 'Sample-spec');
  const commit = await repository(guidelines, { 'guidelines/principles.md': '# Rules\n', 'profiles/dotnet.md': '# .NET\n' });
  git(['tag', 'v0.1.0'], guidelines);
  await repository(code, { 'README.md': '# Code\n' });
  const manifest = {
    schemaVersion: 1, name: 'Sample', code: { repository: '../Sample' },
    guidelines: { repository: '../CretAI', ref: 'v0.1.0' }, profile: 'dotnet',
  };
  const lock = { schemaVersion: 1, ref: 'v0.1.0', commit };
  await repository(spec, {
    'project.json': JSON.stringify(manifest), 'guidelines.lock.json': JSON.stringify(lock),
    'spec/vision.md': '# Spec\n',
  });
  return { root, guidelines, code, spec, manifest, lock, commit };
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
  assert.throws(() => resolveRepository(path.resolve('private-local'), 'https://example.test/spec'), /lokaal/);
});

test('clone produces an independent product clone and fixed guidelines without publishing context', async t => {
  const f = await fixture(t);
  const target = path.join(f.root, 'work spaces', 'My Project');
  await fs.writeFile(path.join(f.guidelines, 'uncommitted.md'), 'Private draft');
  const result = await cloneProject(f.spec, target, { guidelinesRoot: f.guidelines });
  assert.equal(result.manifest.name, 'Sample');
  const codeRoot = path.join(target, 'code');
  assert.equal(git(['status', '--porcelain'], codeRoot), '');
  assert.equal(git(['ls-files'], codeRoot), 'README.md');
  assert.equal(git(['rev-parse', 'HEAD'], path.join(target, '.local', 'guidelines')), f.commit);
  assert.equal(git(['rev-parse', '--abbrev-ref', 'HEAD'], path.join(target, '.local', 'guidelines')), 'HEAD');
  await assert.rejects(fs.access(path.join(target, '.local', 'guidelines', 'uncommitted.md')));
  assert.match(git(['status', '--porcelain'], f.guidelines), /uncommitted.md/);
  const workspace = JSON.parse(await fs.readFile(result.workspaceFile, 'utf8'));
  assert.equal(workspace.folders[2].path, f.guidelines.split(path.sep).join('/'));
  assert.equal(await workspacePath(codeRoot), result.workspaceFile);
  const printed = execFileSync(process.execPath, [cli, 'project', 'open', codeRoot, '--print'], { encoding: 'utf8' }).trim();
  assert.equal(printed, result.workspaceFile);
});

test('an existing destination and product repository are never overwritten', async t => {
  const f = await fixture(t);
  const target = path.join(f.root, 'existing');
  await fs.mkdir(target);
  await fs.writeFile(path.join(target, 'keep.txt'), 'Keep me');
  await assert.rejects(cloneProject(f.spec, target, { guidelinesRoot: f.guidelines }), /bestaat al/);
  assert.equal(await fs.readFile(path.join(target, 'keep.txt'), 'utf8'), 'Keep me');
  const nested = path.join(f.code, 'private-workspace');
  await assert.rejects(cloneProject(f.spec, nested, { guidelinesRoot: f.guidelines }), /buiten bestaande Git/);
  await assert.rejects(fs.access(nested));
});

test('attach respects existing uncommitted code and resolves refs from spec origin', async t => {
  const f = await fixture(t);
  const specCopy = path.join(f.root, 'other place', 'spec');
  git(['clone', '--', f.spec, specCopy]);
  await fs.writeFile(path.join(f.code, 'README.md'), '# User changes\n');
  const target = path.join(f.root, 'attached workspace');
  const result = await attachProject(f.code, specCopy, target, { guidelinesRoot: f.guidelines });
  assert.equal(await fs.readFile(path.join(f.code, 'README.md'), 'utf8'), '# User changes\n');
  assert.match(git(['status', '--porcelain'], f.code), /README.md/);
  assert.equal(git(['status', '--porcelain'], specCopy), '');
  const state = JSON.parse(await fs.readFile(path.join(target, '.cretspec-workspace.json'), 'utf8'));
  assert.equal(state.code, f.code);
  assert.equal(state.spec, specCopy);
  assert.equal(await workspacePath(target), result.workspaceFile);
});

test('moving a guidelines tag cannot silently change locked rules', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.guidelines, 'new.md'), 'Changed rules');
  git(['add', '.'], f.guidelines);
  git(['-c', 'user.name=Test Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-m', 'Changed rules'], f.guidelines);
  git(['tag', '-f', 'v0.1.0'], f.guidelines);
  const target = path.join(f.root, 'moved-tag');
  await assert.rejects(cloneProject(f.spec, target, { guidelinesRoot: f.guidelines }), /wijkt af van de lock/);
  await assert.rejects(fs.access(path.join(target, '.cretspec-workspace.json')));
});

test('an invalid manifest never causes a code clone or an executable spec step', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.spec, 'project.json'), JSON.stringify({ ...f.manifest, run: 'untrusted command' }));
  git(['add', '.'], f.spec);
  git(['-c', 'user.name=Test Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-m', 'Invalid manifest fixture'], f.spec);
  const target = path.join(f.root, 'invalid');
  await assert.rejects(cloneProject(f.spec, target, { guidelinesRoot: f.guidelines }), /vereist precies/);
  await assert.rejects(fs.access(path.join(target, 'code')));
  assert.equal(git(['status', '--porcelain'], f.code), '');
});

test('help and version work without configuration; missing configuration explains setup', async t => {
  const root = await temporary(t);
  const env = { ...process.env, CRETSPEC_HOME: path.join(root, 'personal config') };
  const help = execFileSync(process.execPath, [cli, '--help'], { encoding: 'utf8' });
  assert.match(help, /project clone/);
  assert.match(help, /CretSpec/);
  const version = execFileSync(process.execPath, [cli, '--version'], { encoding: 'utf8', env }).trim();
  const pkg = JSON.parse(await fs.readFile(new URL('../package.json', import.meta.url), 'utf8'));
  assert.equal(version, pkg.version);
  assert.throws(() => execFileSync(process.execPath, [cli, 'guidelines', 'path'], { env, stdio: 'pipe' }), error => {
    assert.match(String(error.stderr), /cspec guidelines set/);
    return true;
  });
  assert.throws(() => execFileSync(process.execPath, [cli, 'project', 'open', '--unsupported'], { stdio: 'pipe' }));
});

test('an installed CLI outside Git uses a separately configured editable guidelines clone', async t => {
  const f = await fixture(t);
  const installed = path.join(f.root, 'installed tool');
  const projectRoot = fileURLToPath(new URL('../', import.meta.url));
  for (const name of ['bin', 'src', 'package.json']) {
    await fs.cp(path.join(projectRoot, name), path.join(installed, name), { recursive: true });
  }
  const env = { ...process.env, CRETSPEC_HOME: path.join(f.root, 'personal config') };
  const run = (...args) => execFileSync(process.execPath, [path.join(installed, 'bin', 'cspec.mjs'), ...args], {
    cwd: f.root, encoding: 'utf8', env, stdio: 'pipe',
  }).trim();
  run('guidelines', 'set', f.guidelines);
  assert.equal(run('guidelines', 'path'), f.guidelines);
  assert.equal(run('guidelines', 'edit', '--print'), f.guidelines);
  const target = path.join(f.root, 'new workspace');
  run('project', 'clone', f.spec, target);
  const state = JSON.parse(await fs.readFile(path.join(target, '.cretspec-workspace.json'), 'utf8'));
  assert.equal(state.editableGuidelines, f.guidelines);
  assert.notEqual(state.editableGuidelines, installed);
  assert.equal(run('project', 'open', target, '--print'), state.workspaceFile);
  assert.equal(git(['status', '--porcelain'], path.join(target, 'code')), '');
  await assert.rejects(fs.access(path.join(installed, '.git')));
});

test('an invalid guidelines selection preserves the previous personal configuration', async t => {
  const f = await fixture(t);
  const configDirectory = path.join(f.root, 'personal config');
  const env = { ...process.env, CRETSPEC_HOME: configDirectory };
  const run = (...args) => execFileSync(process.execPath, [cli, ...args], { env, encoding: 'utf8', stdio: 'pipe' }).trim();
  run('guidelines', 'set', f.guidelines);
  const before = await fs.readFile(path.join(configDirectory, 'config.json'), 'utf8');
  assert.throws(() => run('guidelines', 'set', f.code));
  assert.equal(await fs.readFile(path.join(configDirectory, 'config.json'), 'utf8'), before);
  assert.equal(run('guidelines', 'path'), f.guidelines);
  const relocated = path.join(f.root, 'relocated guidelines');
  await fs.rename(f.guidelines, relocated);
  assert.throws(() => run('guidelines', 'path'), error => {
    assert.match(String(error.stderr), /hernoemen/);
    return true;
  });
  run('guidelines', 'set', relocated);
  assert.equal(run('guidelines', 'path'), relocated);
});

test('a different editable repository cannot silently substitute the pinned guidelines', async t => {
  const f = await fixture(t);
  const unrelated = path.join(f.root, 'unrelated rules');
  await repository(unrelated, { 'guidelines/principles.md': '# Different rules\n', 'profiles/dotnet.md': '# Different profile\n' });
  const target = path.join(f.root, 'wrong configured source');
  await assert.rejects(cloneProject(f.spec, target, { guidelinesRoot: unrelated }), /git fetch --tags/);
  await assert.rejects(fs.access(path.join(target, '.cretspec-workspace.json')));
  assert.equal(git(['status', '--porcelain'], unrelated), '');
  assert.equal(git(['status', '--porcelain'], f.code), '');
});
