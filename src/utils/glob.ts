function escapeRegexCharacter(character: string): string {
  return character.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function escapeCharacterClassCharacter(character: string): string {
  return character.replace(/[\\\]^]/g, '\\$&');
}

export function globToRegExp(pattern: string): RegExp {
  // Match the conventional file-manager "all files" mask, including names
  // without an extension.
  if (pattern === '*.*') return /^[\s\S]*$/iu;

  const characters = Array.from(pattern);
  let source = '^';

  for (let i = 0; i < characters.length; i++) {
    const character = characters[i];
    if (character === '*') {
      source += '[\\s\\S]*';
    } else if (character === '?') {
      source += '[\\s\\S]';
    } else if (character === '[') {
      const closing = characters.indexOf(']', i + 1);
      if (closing > i + 1) {
        const members = characters.slice(i + 1, closing);
        const negated = members[0] === '!' || members[0] === '^';
        if (negated) members.shift();
        const contents = members
          .map((member, index) => {
            if (member === '-' && index > 0 && index < members.length - 1)
              return '-';
            return member === '-'
              ? '\\-'
              : escapeCharacterClassCharacter(member);
          })
          .join('');
        const candidate = `[${negated ? '^' : ''}${contents}]`;
        try {
          new RegExp(candidate, 'iu');
          source += candidate;
          i = closing;
          continue;
        } catch {
          // Treat malformed character classes literally.
        }
      }
      source += '\\[';
    } else {
      source += escapeRegexCharacter(character);
    }
  }

  return new RegExp(`${source}$`, 'iu');
}

export function matchesGlob(value: string, pattern: string): boolean {
  return globToRegExp(pattern).test(value);
}
