export function clampSum(xs: number[], lo: number, hi: number): number {
  if (lo > hi) {
    throw new Error(`clampSum: lo (${lo}) must not be greater than hi (${hi})`);
  }
  return xs.reduce((acc, x) => (x < 0 ? acc : acc + Math.min(Math.max(x, lo), hi)), 0);
}
