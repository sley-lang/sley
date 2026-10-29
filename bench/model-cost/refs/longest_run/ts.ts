export function longestRun(xs: number[]): number {
  let best = 0;
  let run = 0;
  xs.forEach((x, i) => {
    run = i > 0 && x === xs[i - 1] ? run + 1 : 1;
    best = Math.max(best, run);
  });
  return best;
}
