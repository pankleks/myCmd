import {
  readFile,
  writeFile,
  mkdir,
  readdir,
  copyFile,
  stat,
} from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, basename } from 'node:path';

export const bundles = {
  'windows-x64': ['.msi', '.exe'],
  'linux-x64': ['.deb', '.AppImage'],
  'macos-x64': ['.dmg'],
  'macos-arm64': ['.dmg'],
};

export function validateVersion(version) {
  const match =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(
      version,
    );
  if (
    !match ||
    match[4]
      ?.split('.')
      .some((part) => /^\d+$/.test(part) && part.length > 1 && part[0] === '0')
  ) {
    throw new Error(
      `Invalid version: ${version}. Use X.Y.Z or X.Y.Z-rc.1 without build metadata.`,
    );
  }
  return version;
}

async function manifests(root) {
  const paths = [
    'package.json',
    'package-lock.json',
    'src-tauri/tauri.conf.json',
    'src-tauri/Cargo.toml',
    'src-tauri/Cargo.lock',
  ];
  const texts = await Promise.all(
    paths.map((path) => readFile(join(root, path), 'utf8')),
  );
  const [pkg, lock, tauri] = texts.slice(0, 3).map((text) => JSON.parse(text));
  const cargo = /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(texts[3]);
  const cargoLock =
    /^\[\[package\]\]\r?\nname = "mycmd"\r?\nversion = "([^"]+)"/m.exec(
      texts[4],
    );
  if (!cargo || !cargoLock)
    throw new Error('Cannot find mycmd package version in Cargo manifests.');
  const versions = [
    pkg.version,
    lock.version,
    lock.packages?.['']?.version,
    tauri.version,
    cargo[1],
    cargoLock[1],
  ];
  validateVersion(pkg.version);
  if (versions.some((version) => version !== pkg.version))
    throw new Error(
      `Versions are inconsistent: ${versions.join(', ')}. Run npm run release:version -- X.Y.Z.`,
    );
  return {
    paths,
    texts,
    pkg,
    lock,
    tauri,
    cargo,
    cargoLock,
    version: pkg.version,
  };
}

export async function checkVersion(root, tag) {
  const { version } = await manifests(root);
  if (tag !== undefined && tag !== `v${version}`)
    throw new Error(`Tag ${tag} does not match v${version}.`);
  return { version, prerelease: version.includes('-') };
}

export async function setVersion(root, version) {
  validateVersion(version);
  const state = await manifests(root);
  state.pkg.version =
    state.lock.version =
    state.lock.packages[''].version =
    state.tauri.version =
      version;
  const texts = [
    ...[state.pkg, state.lock, state.tauri].map(
      (value) => `${JSON.stringify(value, null, 2)}\n`,
    ),
    state.texts[3].replace(
      state.cargo[0],
      state.cargo[0].replace(/version\s*=\s*"[^"]+"/, `version = "${version}"`),
    ),
    state.texts[4].replace(
      state.cargoLock[0],
      state.cargoLock[0].replace(
        /version\s*=\s*"[^"]+"/,
        `version = "${version}"`,
      ),
    ),
  ];
  await Promise.all(
    state.paths.map((path, index) => writeFile(join(root, path), texts[index])),
  );
}

export async function releaseNotes(root, version) {
  validateVersion(version);
  let changelog;
  try {
    changelog = await readFile(join(root, 'CHANGELOG.md'), 'utf8');
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    // Repositories without a changelog can still validate and publish a
    // release; retain the package/install information in generated notes.
    changelog = `## [${version}]\nRelease ${version}.`;
  }
  const sections = changelog.split(/^## /m);
  const section = sections.find(
    (text) =>
      text.startsWith(`[${version}]`) && /^\[[^\]]+\](?:\s|$)/.test(text),
  );
  const newline = section?.indexOf('\n') ?? -1;
  const notes = newline < 0 ? '' : section.slice(newline + 1).trim();
  if (!notes)
    throw new Error(`Add release notes under ## [${version}] in CHANGELOG.md.`);
  return `# myCmd ${version}\n\n${notes}\n\n## Packages\n\n- Windows x64: MSI / NSIS installer\n- Linux x64: DEB / AppImage\n- macOS Intel and Apple Silicon: DMG\n\nSHA-256 checksums: \`SHA256SUMS.txt\`. Packages are currently unsigned; macOS packages are not notarized.\n`;
}

async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...(await files(path)));
    else if (entry.isFile()) result.push(path);
  }
  return result;
}

export async function collectAssets(bundleDirectory, destination, platform) {
  const extensions = bundles[platform];
  if (!extensions) throw new Error(`Unknown platform: ${platform}`);
  const candidates = (await files(bundleDirectory)).filter((path) =>
    extensions.some((ext) => path.endsWith(ext)),
  );
  for (const extension of extensions)
    if (!candidates.some((path) => path.endsWith(extension)))
      throw new Error(`Missing ${extension} bundle for ${platform}.`);
  const names = candidates.map((path) => `${platform}--${basename(path)}`);
  if (new Set(names).size !== names.length)
    throw new Error(`Duplicate asset names for ${platform}.`);
  for (const path of candidates)
    if (!(await stat(path)).size) throw new Error(`Empty bundle: ${path}`);
  await mkdir(destination, { recursive: true });
  await Promise.all(
    candidates.map((path, index) =>
      copyFile(path, join(destination, names[index])),
    ),
  );
  return names.sort();
}

export async function checksums(directory) {
  const entries = (await readdir(directory))
    .filter((name) =>
      Object.values(bundles)
        .flat()
        .some((ext) => name.endsWith(ext)),
    )
    .sort();
  for (const [platform, extensions] of Object.entries(bundles)) {
    for (const extension of extensions)
      if (
        !entries.some(
          (name) =>
            name.startsWith(`${platform}--`) && name.endsWith(extension),
        )
      )
        throw new Error(`Missing release asset: ${platform} ${extension}`);
  }
  const lines = [];
  for (const name of entries) {
    if (/[\r\n]/.test(name)) throw new Error('Invalid asset filename.');
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(join(directory, name)))
      hash.update(chunk);
    lines.push(`${hash.digest('hex')}  ${name}`);
  }
  const output = `${lines.join('\n')}\n`;
  await writeFile(join(directory, 'SHA256SUMS.txt'), output);
  return output;
}
