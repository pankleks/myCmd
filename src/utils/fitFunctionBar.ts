export function fitFunctionBar(bar: HTMLElement) {
  function fit() {
    const buttons = [...bar.querySelectorAll('button')];
    for (const button of buttons) button.classList.remove('hide-shortcut');
    const overflowing = buttons.filter((button) => {
      const key = button.querySelector('kbd');
      const label = button.querySelector('span');
      if (!key || !label) return false;
      const style = getComputedStyle(button);
      return (
        key.getBoundingClientRect().width +
          label.scrollWidth +
          parseFloat(style.columnGap || '0') +
          parseFloat(style.paddingLeft || '0') +
          parseFloat(style.paddingRight || '0') >
        button.clientWidth
      );
    });
    for (const button of overflowing) button.classList.add('hide-shortcut');
  }
  const observer = new ResizeObserver(fit);
  observer.observe(bar);
  fit();
  return { destroy: () => observer.disconnect() };
}
