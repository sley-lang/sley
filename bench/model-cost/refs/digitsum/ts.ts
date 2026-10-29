export function digitsum(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    for (let x = i; x > 0; x = Math.floor(x / 10)) total += x % 10;
  }
  return total;
}
