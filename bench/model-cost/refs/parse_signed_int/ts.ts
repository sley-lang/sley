export function parseSignedInt(s: string): number {
  if (!/^[+-]?[0-9]+$/.test(s)) {
    throw new Error("invalid integer");
  }
  const negative = s[0] === "-";
  const digits = s[0] === "+" || s[0] === "-" ? s.slice(1) : s;
  const trimmed = digits.replace(/^0+/, "");
  if (trimmed.length > 7) {
    throw new Error("out of range");
  }
  const magnitude = trimmed.length === 0 ? 0 : Number(trimmed);
  if (magnitude > 1000000) {
    throw new Error("out of range");
  }
  if (magnitude === 0) {
    return 0;
  }
  return negative ? -magnitude : magnitude;
}
