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

  if (i === chars.length) {
    throw new Error("parseInteger: no digits after minus sign");
  }

  let value = 0;
  for (; i < chars.length; i++) {
    const code = chars[i];
    if (code < 48 || code > 57) {
      throw new Error(`parseInteger: invalid character code ${code} at index ${i}`);
    }
    value = value * 10 + (code - 48);
  }

  return negative ? -value : value;
}
