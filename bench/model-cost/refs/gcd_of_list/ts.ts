export function gcdOfList(xs: number[]): number {
  let g = 0;
  for (const x of xs) {
    let a = g;
    let b = Math.abs(x);
    while (b !== 0) {
      const t = a % b;
      a = b;
      b = t;
    }
    g = a;
  }
  return g;
}
