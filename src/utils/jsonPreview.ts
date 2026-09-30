import { applyEdits, format, parseTree, type ParseError } from 'jsonc-parser';

export function formatJsonPreview(source: string, jsonc = false): string {
  const errors: ParseError[] = [];
  const tree = parseTree(source, errors, {
    disallowComments: !jsonc,
    allowTrailingComma: jsonc,
  });
  if (!tree || errors.length) throw new Error('Cannot format invalid JSON.');
  return applyEdits(
    source,
    format(source, undefined, {
      tabSize: 2,
      insertSpaces: true,
      eol: '\n',
    }),
  );
}
