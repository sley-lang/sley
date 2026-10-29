export function rootSum(n: number): number {
  const digitRoot = (x: number): number => {
    let v = x;
    while (v >= 10) {
      let s = 0;
      while (v > 0) {
        s += v % 10;
        v = Math.floor(v / 10);
      }
      v = s;
    }
    return v;
  };

  let total = 0;
  for (let i = 1; i <= n; i++) {
    total += digitRoot(i);
  }
  return total;
}
