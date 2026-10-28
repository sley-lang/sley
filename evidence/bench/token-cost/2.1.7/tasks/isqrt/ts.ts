export function isqrt(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    let lo = 0;
    let hi = i + 1;
    while (hi - lo > 1) {
      const mid = Math.floor((lo + hi) / 2);
      if (mid * mid <= i) {
        lo = mid;
      } else {
        hi = mid;
      }
    }
    total += lo;
  }
  return total;
}
