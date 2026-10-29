export function rootSum(n: number): number {
  let total = 0;
  for (let i = 1; i <= n; i++) {
    let x = i;
    while (x >= 10) {
      let s = 0;
      while (x > 0) {
        s += x % 10;
        x = Math.floor(x / 10);
      }
      x = s;
    }
    total += x;
  }
  return total;
}
