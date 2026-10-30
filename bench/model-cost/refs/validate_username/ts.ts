export function validateUsername(s: string): number {
  if (!/^[a-z][a-z0-9_]{2,15}$/.test(s)) {
    return 0;
  }
  if (s.includes("__") || s.endsWith("_")) {
    return 0;
  }
  return 1;
}
