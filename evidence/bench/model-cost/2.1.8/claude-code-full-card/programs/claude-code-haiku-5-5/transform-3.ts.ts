export function transform(xs: number[], lo: number, hi: number): number {
  let total = 0;
  for (const x of xs) {
    const y = x % 2 !== 0 ? 2 * x : x;
    const c = y < lo ? lo : y > hi ? hi : y;
    total += c;
  }
  return total;
}
