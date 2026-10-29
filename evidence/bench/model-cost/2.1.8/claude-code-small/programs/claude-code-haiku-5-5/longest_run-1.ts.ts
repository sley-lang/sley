export function longestRun(xs: number[]): number {
  if (xs.length === 0) {
    return 0;
  }
  let best = 1;
  let current = 1;
  for (let i = 1; i < xs.length; i++) {
    if (xs[i] === xs[i - 1]) {
      current++;
      if (current > best) {
        best = current;
      }
    } else {
      current = 1;
    }
  }
  return best;
}
