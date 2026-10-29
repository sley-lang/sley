export function isqrt(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    let r = Math.floor(Math.sqrt(i));
    // Correct any floating-point rounding error so r is the exact integer square root.
    while (r * r > i) r--;
    while ((r + 1) * (r + 1) <= i) r++;
    total += r;
  }
  return total;
}
