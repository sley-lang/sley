export function transform(xs: number[], lo: number, hi: number): number {
  let sum = 0;
  for (const x of xs) {
    const y = x % 2 !== 0 ? 2 * x : x;
    sum += Math.min(hi, Math.max(lo, y));
  }
  return sum;
}
