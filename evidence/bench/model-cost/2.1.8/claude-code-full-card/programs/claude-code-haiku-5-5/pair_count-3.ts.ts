export function pairCount(xs: number[], k: number): number {
  const seen = new Map<number, number>();
  let count = 0;
  for (const x of xs) {
    const need = k - x;
    count += seen.get(need) ?? 0;
    seen.set(x, (seen.get(x) ?? 0) + 1);
  }
  return count;
}
