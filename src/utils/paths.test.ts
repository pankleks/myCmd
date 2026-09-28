import { describe, expect, it } from 'vitest';
import { displayPath } from './paths';

describe('displayPath', () => {
  it('strips the Windows verbatim prefix', () => {
    expect(displayPath('\\\\?\\D:\\Git')).toBe('D:\\Git');
    expect(displayPath('\\\\?\\C:\\Users\\root')).toBe('C:\\Users\\root');
  });

  it('converts device UNC paths to regular UNC paths', () => {
    expect(displayPath('\\\\?\\UNC\\server\\share')).toBe('\\\\server\\share');
  });

  it('leaves regular paths untouched', () => {
    expect(displayPath('D:\\Git')).toBe('D:\\Git');
    expect(displayPath('/home/user')).toBe('/home/user');
    expect(displayPath('\\\\server\\share')).toBe('\\\\server\\share');
    expect(displayPath('')).toBe('');
  });
});
