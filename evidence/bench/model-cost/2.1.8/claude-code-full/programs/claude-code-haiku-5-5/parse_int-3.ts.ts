export function parseInteger(chars: number[]): number {
  if (chars.length === 0) {
    throw new Error("empty input");
  }

  let i = 0;
  let negative = false;

  if (chars[0] === 45) {
    negative = true;
    i = 1;
    if (chars.length === 1) {
      throw new Error("minus sign without digits");
    }
  }

  let value = 0;
  for (; i < chars.length; i++) {
    const c = chars[i];
    if (c < 48 || c > 57) {
      throw new Error(`invalid character code: ${c}`);
    }
    value = value * 10 + (c - 48);
  }

  return negative ? -value : value;
}
