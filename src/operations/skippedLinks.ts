/** Message after a ZIP compression that left out symbolic links, or undefined when none were skipped. */
export function skippedLinksMessage(
  links: number | undefined,
  resultPath: string | undefined,
): string | undefined {
  if (!links) return undefined;
  if (!resultPath)
    return 'No archive was created: all selected items are symbolic links, which ZIP archives do not store.';
  return `${links} symbolic link${links === 1 ? ' was' : 's were'} not added to the ZIP. ZIP archives do not store links.`;
}
