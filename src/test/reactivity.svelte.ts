/** Give rune-based tests an owner, as mounted Svelte components have. */
export function inReactiveRoot(run: () => void): void {
  const dispose = $effect.root(run);
  dispose();
}
