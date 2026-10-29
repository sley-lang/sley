export function parseInteger(chars: number[]): number {
  if (chars.length === 0) throw new Error("empty");
  let sign = 1;
  let start = 0;
  if (chars[0] === 45) {
    if (chars.length === 1) throw new Error("lone minus");
    sign = -1;
    start = 1;
  }
  let value = 0;
  for (const c of chars.slice(start)) {
    if (c < 48 || c > 57) throw new Error("not a digit");
    value = value * 10 + (c - 48);
  }
  return sign * value;
}
