export function transform(xs: number[], lo: number, hi: number): number {
  return xs
    .map((x) => Math.min(Math.max(x % 2 !== 0 ? x * 2 : x, lo), hi))
    .reduce((a, b) => a + b, 0);
}
