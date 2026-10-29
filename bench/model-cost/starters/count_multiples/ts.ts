export function countMultiples(xs: number[], k: number): number {
  return xs.filter((x) => x % 2 === 0).length;
}
