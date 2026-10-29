export function parseInteger(chars: number[]): number {
  if (chars.length === 0) {
    throw new Error("parseInteger: empty input");
  }

  let i = 0;
  let negative = false;
  if (chars[0] === 45) {
    negative = true;
    i = 1;
  }

  if (i >= chars.length) {
    throw new Error("parseInteger: no digits");
  }

  let value = 0;
  for (; i < chars.length; i++) {
    const c = chars[i];
    if (c < 48 || c > 57) {
      throw new Error(`parseInteger: invalid character code ${c}`);
    }
    value = value * 10 + (c - 48);
  }

  return negative ? -value : value;
}
