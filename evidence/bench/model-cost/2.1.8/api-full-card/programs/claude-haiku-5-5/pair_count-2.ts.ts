export function pairCount(xs: number[], k: number): number {
  const seen = new Map<number, number>();
  let count = 0;
  for (const x of xs) {
    const need = k - x;
    const c = seen.get(need);
    if (c !== undefined) {
      count += c;
    }
    seen.set(x, (seen.get(x) ?? 0) + 1);
  }
  return count;
}
