import { describe, expect, it } from 'vitest';
import { cdTarget } from './cdCommand';

describe('standalone directory commands', () => {
  it('resolves relative and parent paths against the panel', () => {
    expect(cdTarget('cd tools', 'C:\\work')).toBe('C:\\work\\tools');
    expect(cdTarget('cd ..', '/home/work')).toBe('/home/work/..');
    expect(cdTarget('CD /D "D:\\My Files"', 'C:\\work')).toBe('D:\\My Files');
    expect(cdTarget('chdir "my folder"', '/home')).toBe('/home/my folder');
    expect(cdTarget('cd \\tools', 'C:\\work')).toBe('C:\\tools');
    expect(cdTarget('cd "a&b"', '/home')).toBe('/home/a&b');
  });
  it('leaves queries, expansions and compound commands to the shell', () => {
    for (const command of [
      'cd',
      'echo hi',
      'cd foo && dir',
      'cd foo; ls',
      'cd %TEMP%',
      'cd $HOME',
      'cd D:foo',
    ]) {
      expect(cdTarget(command, 'C:\\work')).toBeUndefined();
    }
  });
});
