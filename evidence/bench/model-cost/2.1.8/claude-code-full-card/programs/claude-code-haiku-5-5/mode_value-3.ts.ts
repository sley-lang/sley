export function modeValue(xs: number[]): number {
  if (xs.length === 0) {
    throw new Error("modeValue: empty input");
  }

  const counts = new Map<number, number>();
  for (const x of xs) {
    counts.set(x, (counts.get(x) ?? 0) + 1);
  }

  let best = 0;
  let bestCount = 0;
  let first = true;
  for (const [value, count] of counts) {
    if (
      first ||
      count > bestCount ||
      (count === bestCount && value < best)
    ) {
      best = value;
      bestCount = count;
      first = false;
    }
  }

  return best;
}
