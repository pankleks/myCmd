import { expect, it } from 'vitest';
import data from '../file-icons.generated.json';
import { fileIcon } from './fileIcon';

const icons: Record<string, (typeof data.icons)['_file']> = data.icons;

it('uses the bundled Zed theme for names, compound suffixes, folders and fallbacks', () => {
  expect(fileIcon('README.md')).toBe(data.icons.readme);
  expect(fileIcon('Cargo.toml')).toBe(data.icons.cargo);
  expect(fileIcon('APP.TS')).toBe(data.icons.typescript);
  expect(fileIcon('app.component.ts')).toBe(data.icons['angular-component']);
  expect(fileIcon('src', true)).toBe(icons[data.directories.src]);
  expect(fileIcon('unknown', true)).toBe(icons[data.defaultFolder]);
  expect(fileIcon('..', true, true)).toBe(icons[data.parentFolder]);
  expect(fileIcon('unknown.unrecognized')).toBe(icons[data.defaultFile]);
  for (const key of [
    ...Object.values(data.filenames),
    ...Object.values(data.extensions),
    ...Object.values(data.directories),
    data.defaultFile,
    data.defaultFolder,
    data.parentFolder,
  ])
    expect(data.icons).toHaveProperty(key);
});
