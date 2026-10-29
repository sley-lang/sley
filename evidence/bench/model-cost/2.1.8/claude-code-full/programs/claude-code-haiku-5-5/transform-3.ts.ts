export function transform(xs: number[], lo: number, hi: number): number {
  let total = 0;
  for (const x of xs) {
    const mapped = Math.abs(x % 2) === 1 ? 2 * x : x;
    const clamped = Math.min(hi, Math.max(lo, mapped));
    total += clamped;
  }
  return total;
}
