// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Icon from './Icon.svelte';

const mounted: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.innerHTML = '';
});
function show(icon: { body: string; width?: number; height?: number }) {
  const target = document.createElement('div');
  document.body.append(target);
  mounted.push(mount(Icon, { target, props: { icon, width: 16, height: 16 } }));
  flushSync();
  return target.querySelector('svg')!;
}

it('draws the body as SVG in the icon box', () => {
  const svg = show({ body: '<path d="M0 0h24"/>', width: 24, height: 24 });
  expect(svg.getAttribute('viewBox')).toBe('0 0 24 24');
  expect(svg.getAttribute('width')).toBe('16');
  expect(svg.getAttribute('aria-hidden')).toBe('true');
  const path = svg.querySelector('path')!;
  expect(path.namespaceURI).toBe('http://www.w3.org/2000/svg');
});

it('gives each icon instance its own gradient and mask IDs', () => {
  const body =
    '<defs><linearGradient id="a"/></defs><path fill="url(#a)"/><use href="#a"/>';
  const first = show({ body });
  const second = show({ body });
  const ids = [first, second].map((svg) => svg.querySelector('[id]')!.id);
  expect(ids[0]).not.toBe(ids[1]);
  expect(first.querySelector('path')!.getAttribute('fill')).toBe(
    `url(#${ids[0]})`,
  );
  expect(first.querySelector('use')!.getAttribute('href')).toBe(`#${ids[0]}`);
});
