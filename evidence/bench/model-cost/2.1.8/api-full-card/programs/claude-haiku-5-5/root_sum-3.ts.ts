export function rootSum(n: number): number {
  let total = 0;
  for (let i = 1; i <= n; i++) {
    total += i % 9 === 0 ? 9 : i % 9;
  }
  return total;
}
