export function loopsum(n: number): number {
  let s = 0;
  for (let i = 0; i < n; i++) {
    s = (s + i * i) % 1000000007;
  }
  return s;
}
