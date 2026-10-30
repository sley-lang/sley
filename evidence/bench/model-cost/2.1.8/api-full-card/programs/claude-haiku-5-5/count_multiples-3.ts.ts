export function countMultiples(xs: number[], k: number): number {
  if (!(k > 0)) {
    throw new Error("k must be a positive number");
  }
  return xs.filter((x) => x % k === 0).length;
}
