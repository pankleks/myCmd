import { expect, it } from 'vitest';
import { skippedLinksMessage } from './skippedLinks';

it('reports skipped links only when there were any', () => {
  expect(skippedLinksMessage(undefined, '/a.zip')).toBeUndefined();
  expect(skippedLinksMessage(0, '/a.zip')).toBeUndefined();
  expect(skippedLinksMessage(1, '/a.zip')).toBe(
    '1 symbolic link was not added to the ZIP. ZIP archives do not store links.',
  );
  expect(skippedLinksMessage(3, '/a.zip')).toContain('3 symbolic links were');
  expect(skippedLinksMessage(2, undefined)).toContain('No archive was created');
});
