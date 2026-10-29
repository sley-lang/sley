export function pairCount(xs: number[], k: number): number {
  const seen = new Map<number, number>();
  let total = 0;
  for (const x of xs) {
    const need = k - x;
    total += seen.get(need) ?? 0;
    seen.set(x, (seen.get(x) ?? 0) + 1);
  }
  return total;
}
