export function longestIncreasingRun(xs: number[]): number {
  if (xs.length === 0) {
    return 0;
  }
  let best = 1;
  let current = 1;
  for (let i = 1; i < xs.length; i++) {
    current = xs[i] > xs[i - 1] ? current + 1 : 1;
    best = Math.max(best, current);
  }
  return best;
}
