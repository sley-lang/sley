export function topKFrequent(xs: number[], k: number): number[] {
  const counts = new Map<number, number>();
  for (const x of xs) {
    counts.set(x, (counts.get(x) ?? 0) + 1);
  }
  const values = [...counts.keys()];
  values.sort((a, b) => {
    const diff = (counts.get(b) as number) - (counts.get(a) as number);
    return diff !== 0 ? diff : a - b;
  });
  return values.slice(0, Math.max(0, k));
}
