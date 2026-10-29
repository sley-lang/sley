export function rootSum(n: number): number {
  let total = 0;
  for (let k = 1; k <= n; k++) {
    let x = k;
    while (x >= 10) {
      let s = 0;
      let y = x;
      while (y > 0) {
        s += y % 10;
        y = Math.floor(y / 10);
      }
      x = s;
    }
    total += x;
  }
  return total;
}
