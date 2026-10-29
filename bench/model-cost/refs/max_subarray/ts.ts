export function maxSubarray(xs: number[]): number {
  if (xs.length === 0) throw new Error("empty");
  let best = xs[0];
  let cur = xs[0];
  for (const x of xs.slice(1)) {
    cur = Math.max(x, cur + x);
    best = Math.max(best, cur);
  }
  return best;
}
