export function denseRank(xs: number[]): number[] {
  const distinct = [...new Set(xs)].sort((a, b) => b - a);
  const rank = new Map<number, number>();
  distinct.forEach((v, i) => rank.set(v, i + 1));
  return xs.map((x) => rank.get(x) as number);
}
