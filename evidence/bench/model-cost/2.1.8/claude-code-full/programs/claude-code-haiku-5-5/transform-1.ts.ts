export function transform(xs: number[], lo: number, hi: number): number {
  let sum = 0;
  for (const x of xs) {
    const mapped = Math.abs(x % 2) === 1 ? 2 * x : x;
    sum += Math.min(hi, Math.max(lo, mapped));
  }
  return sum;
}
