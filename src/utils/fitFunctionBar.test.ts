// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { fitFunctionBar } from './fitFunctionBar';

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

it('hides shortcuts only on overflowing buttons and restores them after resizing', () => {
  let resize!: () => void;
  const disconnect = vi.fn();
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(callback: () => void) {
        resize = callback;
      }
      observe() {}
      disconnect = disconnect;
    },
  );
  const bar = document.createElement('footer');
  bar.innerHTML =
    '<button style="column-gap:6px;padding:0 4px"><kbd>F1</kbd><span>Help</span></button><button style="column-gap:6px;padding:0 4px"><kbd>Alt+Enter</kbd><span>Properties</span></button>';
  document.body.append(bar);
  let width = 110;
  for (const [index, button] of [...bar.querySelectorAll('button')].entries()) {
    Object.defineProperty(button, 'clientWidth', { get: () => width });
    Object.defineProperty(button.querySelector('span'), 'scrollWidth', {
      value: index ? 65 : 25,
    });
    button.querySelector('kbd')!.getBoundingClientRect = () =>
      ({ width: index ? 60 : 15 }) as DOMRect;
  }
  const action = fitFunctionBar(bar);
  const buttons = bar.querySelectorAll('button');
  expect(buttons[0].classList.contains('hide-shortcut')).toBe(false);
  expect(buttons[1].classList.contains('hide-shortcut')).toBe(true);
  width = 150;
  resize();
  expect(buttons[1].classList.contains('hide-shortcut')).toBe(false);
  width = 100;
  resize();
  expect(buttons[1].classList.contains('hide-shortcut')).toBe(true);
  action.destroy();
  expect(disconnect).toHaveBeenCalledOnce();
});
