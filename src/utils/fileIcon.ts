import data from '../file-icons.generated.json';

const filenames: Record<string, string> = data.filenames;
const suffixes: Record<string, string> = data.extensions;
const directories: Record<string, string> = data.directories;
const icons: Record<string, { body: string; width: number; height: number }> =
  data.icons;

export function fileIcon(name: string, directory = false, parent = false) {
  name = name.toLowerCase();
  if (parent) return icons[data.parentFolder];
  if (directory) return icons[directories[name] ?? data.defaultFolder];
  const dot = name.lastIndexOf('.');
  const named =
    filenames[name] ?? filenames[dot > 0 ? name.slice(0, dot) : name];
  if (named) return icons[named];
  // Try compound suffixes first: component.ts before ts.
  while (name) {
    if (suffixes[name]) return icons[suffixes[name]];
    const dot = name.indexOf('.');
    if (dot < 0) break;
    name = name.slice(dot + 1);
  }
  return icons[data.defaultFile];
}
