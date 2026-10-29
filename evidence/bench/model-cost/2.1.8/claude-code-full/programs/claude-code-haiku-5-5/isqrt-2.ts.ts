export function isqrt(n: number): number {
  let total = 0;
  let r = 0;
  for (let i = 0; i < n; i++) {
    while ((r + 1) * (r + 1) <= i) {
      r++;
    }
    total += r;
  }
  return total;
}
