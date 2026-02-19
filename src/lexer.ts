export type Token =
  | { kind: 'kw'; value: 'fn' | 'let' | 'if' | 'else' | 'while' | 'return' | 'export' | 'const' }
  | { kind: 'ident'; value: string }
  | { kind: 'num'; value: number }
  | { kind: 'sym'; value: string }
  | { kind: 'eof' };

const isAlpha = (c: string) => /[A-Za-z_]/.test(c);
const isAlnum = (c: string) => /[A-Za-z0-9_]/.test(c);
const isDigit = (c: string) => /[0-9]/.test(c);

export function lex(src: string): Token[] {
  const t: Token[] = [];
  let i = 0;

  const peek = () => src[i] ?? '';
  const next = () => src[i++] ?? '';

  while (i < src.length) {
    const c = peek();

    if (c === ' ' || c === '\t' || c === '\n' || c === '\r') {
      next();
      continue;
    }

    // line comment
    if (c === '/' && src[i + 1] === '/') {
      while (i < src.length && peek() !== '\n') next();
      continue;
    }

    if (isAlpha(c)) {
      let s = '';
      while (isAlnum(peek())) s += next();
      if (s === 'fn' || s === 'let' || s === 'if' || s === 'else' || s === 'while' || s === 'return' || s === 'export' || s === 'const') {
        t.push({ kind: 'kw', value: s as any });
      } else {
        t.push({ kind: 'ident', value: s });
      }
      continue;
    }

    if (isDigit(c)) {
      let s = '';
      while (isDigit(peek())) s += next();
      t.push({ kind: 'num', value: Number(s) });
      continue;
    }

    // two-char operators
    const two = src.slice(i, i + 2);
    const twoSyms = ['==', '!=', '<=', '>=', '<<', '>>'];
    if (twoSyms.includes(two)) {
      t.push({ kind: 'sym', value: two });
      i += 2;
      continue;
    }

    const singles = ['(', ')', '{', '}', ':', ';', ',', '+', '-', '*', '&', '|', '^', '<', '>', '=', '!', '@'];
    if (singles.includes(c)) {
      t.push({ kind: 'sym', value: next() });
      continue;
    }

    throw new Error(`Unexpected character '${c}' at ${i}`);
  }

  t.push({ kind: 'eof' });
  return t;
}
