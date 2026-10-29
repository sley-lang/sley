export function rootSum(n: number): number {
  let total = 0;
  for (let k = 1; k <= n; k++) {
    // Digital root of a positive integer k is 1 + (k - 1) mod 9.
    total += 1 + ((k - 1) % 9);
  }
  return total;
}
