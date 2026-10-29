export function parseInteger(chars: number[]): number {
  if (chars.length === 0) {
    throw new Error("parseInteger: empty input");
  }

  let index = 0;
  let negative = false;

  if (chars[0] === 45) {
    negative = true;
    index = 1;
  }

  if (index >= chars.length) {
    throw new Error("parseInteger: no digits after minus sign");
  }

  let value = 0;
  for (; index < chars.length; index++) {
    const code = chars[index];
    if (code < 48 || code > 57) {
      throw new Error(`parseInteger: invalid character code ${code} at position ${index}`);
    }
    value = value * 10 + (code - 48);
  }

  return negative ? -value : value;
}
