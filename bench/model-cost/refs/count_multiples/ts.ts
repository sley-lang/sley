export function countMultiples(xs: number[], k: number): number {
  if (k <= 0) throw new Error("k must be positive");
  return xs.filter((x) => x % k === 0).length;
}
