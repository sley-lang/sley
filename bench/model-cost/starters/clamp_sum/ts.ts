export function clampSum(xs: number[], lo: number, hi: number): number {
  return xs.reduce((acc, x) => acc + Math.min(Math.max(x, lo), hi), 0);
}
