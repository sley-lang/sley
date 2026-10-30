export function rootSum(n: number): number {
  let total = 0;
  for (let i = 1; i <= n; i++) {
    total += 1 + (i - 1) % 9;
  }
  return total;
}
