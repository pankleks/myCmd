/** Strip only the extension actually displayed in the dedicated Ext column. */
export function fileNameWithoutExtension(
  name: string,
  extension: string,
): string {
  if (!extension) return name;
  const suffix = `.${extension}`;
  return name.length > suffix.length &&
    name.toLowerCase().endsWith(suffix.toLowerCase())
    ? name.slice(0, -suffix.length)
    : name;
}
