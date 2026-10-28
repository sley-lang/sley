export function digitsum(n: number): number {
  let total = 0;
  for (let i = 0; i < n; i++) {
    let x = i;
    while (x > 0) {
      total += x % 10;
      x = Math.floor(x / 10);
    }
  }
  return total;
}
