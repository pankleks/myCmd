// Import once from a Zed extension; normal builds use the bundled JSON.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const root = process.argv[2];
if (!root)
  throw new Error(
    'Usage: npm run generate:file-icons -- <catppuccin-icons extension directory>',
  );
const { themes } = JSON.parse(
  await readFile(resolve(root, 'icon_themes/catppuccin-icons.json'), 'utf8'),
);
const theme = themes.find((theme) => theme.name === 'Catppuccin Macchiato');
const icons = {};
async function importIcon(path) {
  const key = path
    .split('/')
    .at(-1)
    .replace(/\.svg$/, '');
  if (!icons[key]) {
    const svg = await readFile(resolve(root, path), 'utf8');
    const [, width, height] = svg.match(/viewBox="0 0 ([\d.]+) ([\d.]+)"/);
    icons[key] = {
      body: svg
        .replace(/^[\s\S]*?<svg\b[^>]*>/, '')
        .replace(/<\/svg>\s*$/, '')
        .trim(),
      width: Number(width),
      height: Number(height),
    };
  }
  return key;
}
const fileKeys = {};
for (const [key, icon] of Object.entries(theme.file_icons)) {
  fileKeys[key] = await importIcon(icon.path);
}
const mapFiles = (mapping) =>
  Object.fromEntries(
    Object.entries(mapping).map(([name, key]) => [
      name.toLowerCase(),
      fileKeys[key],
    ]),
  );
const directories = {};
for (const [name, icon] of Object.entries(theme.named_directory_icons)) {
  directories[name.toLowerCase()] = await importIcon(icon.collapsed);
}
const output = {
  source:
    'Catppuccin Macchiato — catppuccin/zed-icons 1.24.0 (MIT; see licenses/catppuccin-icons.txt)',
  license: await readFile(
    new URL('../public/licenses/catppuccin-icons.txt', import.meta.url),
    'utf8',
  ),
  extensions: mapFiles(theme.file_suffixes),
  filenames: mapFiles(theme.file_stems),
  directories,
  defaultFile: await importIcon('./icons/macchiato/_file.svg'),
  defaultFolder: await importIcon(theme.directory_icons.collapsed),
  parentFolder: await importIcon(theme.directory_icons.expanded),
  icons,
};
await writeFile(
  new URL('../src/file-icons.generated.json', import.meta.url),
  `${JSON.stringify(output, null, 2)}\n`,
);
