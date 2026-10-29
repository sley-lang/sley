export function longestRun(xs: number[]): number {
  let best = 0;
  let current = 0;
  for (let i = 0; i < xs.length; i++) {
    if (i > 0 && xs[i] === xs[i - 1]) {
      current++;
    } else {
      current = 1;
    }
    if (current > best) {
      best = current;
    }
  }
  return best;
}
