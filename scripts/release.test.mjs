import { afterEach, expect, it } from 'vitest';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import {
  bundles,
  checkVersion,
  setVersion,
  releaseNotes,
  validateVersion,
  collectAssets,
  checksums,
} from './release-lib.mjs';

const directories = [];
afterEach(async () => {
  await Promise.all(
    directories
      .splice(0)
      .map((path) => rm(path, { recursive: true, force: true })),
  );
});
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), 'mycmd-release-'));
  directories.push(root);
  await mkdir(join(root, 'src-tauri'));
  const files = {
    'package.json': JSON.stringify({ name: 'mycmd', version: '0.1.0' }),
    'package-lock.json': JSON.stringify({
      version: '0.1.0',
      packages: {
        '': { version: '0.1.0' },
        'node_modules/example': { version: '9.0.0' },
      },
    }),
    'src-tauri/tauri.conf.json': JSON.stringify({ version: '0.1.0' }),
    'src-tauri/Cargo.toml':
      '[package]\nname = "mycmd"\nversion = "0.1.0"\n\n[dependencies]\nexample = "9.0.0"\n',
    'src-tauri/Cargo.lock':
      'version = 4\n\n[[package]]\nname = "example"\nversion = "9.0.0"\n\n[[package]]\nname = "mycmd"\nversion = "0.1.0"\n',
    'CHANGELOG.md':
      '# Changelog\n\n## [Unreleased]\n\nFuture work.\n\n## [0.1.0] - 2026-09-27\n\nInitial release.\n\n## [0.0.1]\n\nOld notes.\n',
  };
  await Promise.all(
    Object.entries(files).map(([path, value]) =>
      writeFile(join(root, path), value),
    ),
  );
  return root;
}
it.each(['0.1.0', '1.2.3-rc.1', '1.2.3-beta', '10.20.30'])(
  'accepts package-compatible version %s',
  (version) => expect(validateVersion(version)).toBe(version),
);
it.each([
  'v1.2.3',
  '01.2.3',
  '1.2',
  '1.2.3-01',
  '1.2.3+build',
  '1.2.3\nmalicious',
  '../1.2.3',
  '1.2.3-',
])('rejects invalid version %s', (version) =>
  expect(() => validateVersion(version)).toThrow('Invalid version'),
);
it('checks matching manifests and tag, then detects a mismatched tag', async () => {
  const root = await fixture();
  expect(await checkVersion(root, 'v0.1.0')).toEqual({
    version: '0.1.0',
    prerelease: false,
  });
  await expect(checkVersion(root, 'v0.2.0')).rejects.toThrow('does not match');
});
it.each([
  'package.json',
  'package-lock.json',
  'src-tauri/tauri.conf.json',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
])('detects version drift in %s', async (path) => {
  const root = await fixture();
  const text = await readFile(join(root, path), 'utf8');
  await writeFile(join(root, path), text.replaceAll('0.1.0', '0.2.0'));
  await expect(checkVersion(root)).rejects.toThrow('inconsistent');
});
it('rejects Cargo files without the application package', async () => {
  const root = await fixture();
  await writeFile(join(root, 'src-tauri/Cargo.lock'), 'version = 4\n');
  await expect(checkVersion(root)).rejects.toThrow('Cannot find');
});
it('updates all manifests and lockfiles without changing dependencies', async () => {
  const root = await fixture();
  await setVersion(root, '1.2.0-rc.1');
  expect(await checkVersion(root, 'v1.2.0-rc.1')).toEqual({
    version: '1.2.0-rc.1',
    prerelease: true,
  });
  expect(await readFile(join(root, 'src-tauri/Cargo.lock'), 'utf8')).toContain(
    'name = "example"\nversion = "9.0.0"',
  );
  expect(
    JSON.parse(await readFile(join(root, 'package-lock.json'), 'utf8'))
      .packages['node_modules/example'].version,
  ).toBe('9.0.0');
  await expect(setVersion(root, 'invalid')).rejects.toThrow('Invalid version');
});
it('extracts only the selected release notes and requires a changelog entry', async () => {
  const root = await fixture();
  const notes = await releaseNotes(root, '0.1.0');
  expect(notes).toContain('Initial release.');
  expect(notes).not.toContain('Old notes');
  expect(notes).not.toContain('Future work');
  await expect(releaseNotes(root, '0.2.0')).rejects.toThrow('CHANGELOG');
  await writeFile(join(root, 'CHANGELOG.md'), '# Changelog\n\n## [0.1.0]');
  await expect(releaseNotes(root, '0.1.0')).rejects.toThrow('CHANGELOG');
});
it('collects all bundle types, prefixes architecture and produces independently verifiable checksums', async () => {
  const root = await fixture();
  const destination = join(root, 'assets');
  for (const [platform, extensions] of Object.entries(bundles)) {
    const bundle = join(root, platform);
    await mkdir(join(bundle, 'nested'), { recursive: true });
    for (const extension of extensions)
      await writeFile(
        join(bundle, 'nested', `installer${extension}`),
        `binary ${platform}`,
      );
    await writeFile(join(bundle, 'not-an-installer.txt'), 'ignore');
    const names = await collectAssets(bundle, destination, platform);
    expect(names).toHaveLength(extensions.length);
  }
  const output = await checksums(destination);
  expect(output.trim().split('\n')).toHaveLength(6);
  for (const line of output.trim().split('\n')) {
    const [hash, name] = line.split('  ');
    expect(hash).toBe(
      createHash('sha256')
        .update(await readFile(join(destination, name)))
        .digest('hex'),
    );
  }
  expect(await checksums(destination)).toBe(output);
});
it('refuses missing, duplicate, empty or unknown bundles and incomplete releases', async () => {
  const root = await fixture();
  const output = join(root, 'output');
  await expect(collectAssets(root, output, 'unknown')).rejects.toThrow(
    'Unknown',
  );
  await expect(collectAssets(root, output, 'windows-x64')).rejects.toThrow(
    'Missing',
  );
  await writeFile(join(root, 'installer.dmg'), '');
  await expect(collectAssets(root, output, 'macos-x64')).rejects.toThrow(
    'Empty',
  );
  await writeFile(join(root, 'installer.dmg'), 'binary');
  await mkdir(join(root, 'duplicate'));
  await writeFile(join(root, 'duplicate/installer.dmg'), 'binary');
  await expect(collectAssets(root, output, 'macos-x64')).rejects.toThrow(
    'Duplicate',
  );
  await expect(checksums(root)).rejects.toThrow('Missing release asset');
});
