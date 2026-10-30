export function isqrt(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    let r = Math.floor(Math.sqrt(i));
    while (r * r > i) r--;
    while ((r + 1) * (r + 1) <= i) r++;
    total += r;
  }
  return total;
}
