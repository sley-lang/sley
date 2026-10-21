export function gcd(a: number, b: number): number {
  while (b !== 0) {
    [a, b] = [b, a % b];
  }
  return a;
}

export function gcdsum(n: number): number {
  let s = 0;
  for (let i = 1; i < n; i++) {
    for (let j = 1; j < n; j++) {
      s += gcd(i, j);
    }
  }
  return s;
}
