export function transform(xs: number[], lo: number, hi: number): number {
  return xs.reduce((acc, x) => acc + Math.min(Math.max(x % 2 !== 0 ? 2 * x : x, lo), hi), 0);
}
