export function countMultiples(xs: number[], k: number): number {
  if (!Number.isFinite(k) || k <= 0) {
    throw new Error("k must be a positive finite number");
  }
  return xs.filter((x) => x % k === 0).length;
}
