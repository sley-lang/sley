export function clampSum(xs: number[], lo: number, hi: number): number {
  if (lo > hi) throw new Error("lo > hi");
  return xs.filter((x) => x >= 0).reduce((acc, x) => acc + Math.min(Math.max(x, lo), hi), 0);
}
