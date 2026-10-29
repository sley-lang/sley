export function isqrt(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) total += Math.floor(Math.sqrt(i));
  return total;
}
