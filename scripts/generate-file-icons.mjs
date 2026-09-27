import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { icons as vscodeIconSet, info } from '@iconify-json/vscode-icons';
import {
  icons as lucideIconSet,
  info as lucideInfo,
} from '@iconify-json/lucide';

const iconByExtension = {
  pdf: 'file-type-pdf2',
  doc: 'file-type-word2',
  docx: 'file-type-word2',
  docm: 'file-type-word2',
  dot: 'file-type-word2',
  dotx: 'file-type-word2',
  odt: 'file-type-word2',
  rtf: 'file-type-word2',
  xls: 'file-type-excel2',
  xlsx: 'file-type-excel2',
  xlsm: 'file-type-excel2',
  xlt: 'file-type-excel2',
  ods: 'file-type-excel2',
  csv: 'file-type-excel2',
  ppt: 'file-type-powerpoint2',
  pptx: 'file-type-powerpoint2',
  pptm: 'file-type-powerpoint2',
  pps: 'file-type-powerpoint2',
  ppsx: 'file-type-powerpoint2',
  odp: 'file-type-powerpoint2',
  txt: 'file-type-text',
  log: 'file-type-log',
  md: 'file-type-markdown',
  markdown: 'file-type-markdown',
  js: 'file-type-js-official',
  jsx: 'file-type-js',
  mjs: 'file-type-js-official',
  cjs: 'file-type-js-official',
  ts: 'file-type-typescript-official',
  tsx: 'file-type-typescript-official',
  mts: 'file-type-typescript-official',
  cts: 'file-type-typescript-official',
  rs: 'file-type-rust',
  py: 'file-type-python',
  pyw: 'file-type-python',
  go: 'file-type-go',
  java: 'file-type-java',
  c: 'file-type-c',
  h: 'file-type-cppheader',
  cc: 'file-type-cpp',
  cpp: 'file-type-cpp',
  cxx: 'file-type-cpp',
  hpp: 'file-type-cppheader',
  cs: 'file-type-csharp2',
  php: 'file-type-php2',
  swift: 'file-type-swift',
  html: 'file-type-html',
  htm: 'file-type-html',
  css: 'file-type-css2',
  scss: 'file-type-scss',
  sass: 'file-type-scss',
  less: 'file-type-css',
  json: 'file-type-json2',
  jsonc: 'file-type-json2',
  json5: 'file-type-json2',
  xml: 'file-type-xml',
  yaml: 'file-type-yaml-official',
  yml: 'file-type-yaml-official',
  toml: 'file-type-toml',
  sql: 'file-type-sql',
  sh: 'file-type-shell',
  bash: 'file-type-shell',
  zsh: 'file-type-shell',
  ps1: 'file-type-powershell',
  bat: 'file-type-bat',
  cmd: 'file-type-bat',
  png: 'file-type-image',
  jpg: 'file-type-image',
  jpeg: 'file-type-image',
  gif: 'file-type-image',
  svg: 'file-type-svg',
  webp: 'file-type-webp',
  bmp: 'file-type-image',
  tif: 'file-type-image',
  tiff: 'file-type-image',
  ico: 'file-type-image',
  mp3: 'file-type-audio',
  wav: 'file-type-audio',
  flac: 'file-type-audio',
  aac: 'file-type-audio',
  ogg: 'file-type-audio',
  m4a: 'file-type-audio',
  mp4: 'file-type-video',
  mov: 'file-type-video',
  avi: 'file-type-video',
  mkv: 'file-type-video',
  webm: 'file-type-video',
  zip: 'file-type-zip2',
  rar: 'file-type-zip2',
  '7z': 'file-type-zip2',
  tar: 'file-type-zip2',
  gz: 'file-type-zip2',
  bz2: 'file-type-zip2',
  xz: 'file-type-zip2',
  exe: 'file-type-binary',
  dll: 'file-type-binary',
  dmg: 'file-type-binary',
  msi: 'file-type-binary',
  bin: 'file-type-binary',
  wasm: 'file-type-wasm',
  dockerfile: 'file-type-docker',
};
const iconByFilename = {
  dockerfile: 'file-type-docker',
  makefile: 'file-type-shell',
  '.gitignore': 'file-type-git',
  '.gitattributes': 'file-type-git',
  '.gitmodules': 'file-type-git',
  license: 'file-type-license',
  copying: 'file-type-license',
};

const iconNames = new Set([
  'default-file',
  ...Object.values(iconByExtension),
  ...Object.values(iconByFilename),
]);
const selectedIcons = {};
for (const name of iconNames) {
  const icon = vscodeIconSet.icons[name];
  if (!icon) throw new Error(`Missing VSCode icon: ${name}`);
  selectedIcons[name] = {
    ...icon,
    width: info.height,
    height: info.height,
  };
}
for (const [name, source] of Object.entries({
  'folder-outline': 'folder',
  'folder-up-outline': 'folder-up',
})) {
  const icon = lucideIconSet.icons[source];
  if (!icon) throw new Error(`Missing Lucide icon: ${source}`);
  selectedIcons[name] = {
    ...icon,
    width: icon.width ?? lucideInfo.height,
    height: icon.height ?? lucideInfo.height,
  };
}

const output = {
  extensions: iconByExtension,
  filenames: iconByFilename,
  icons: selectedIcons,
};
const destination = fileURLToPath(
  new URL('../src/file-icons.generated.json', import.meta.url),
);
await writeFile(destination, `${JSON.stringify(output, null, 2)}\n`);
