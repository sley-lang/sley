export function transform(xs: number[], lo: number, hi: number): number {
  let sum = 0;
  for (const x of xs) {
    const mapped = (((x % 2) + 2) % 2 === 1) ? 2 * x : x;
    const clamped = Math.min(hi, Math.max(lo, mapped));
    sum += clamped;
  }
  return sum;
}
