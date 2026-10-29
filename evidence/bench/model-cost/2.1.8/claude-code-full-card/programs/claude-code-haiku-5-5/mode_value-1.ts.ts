export function modeValue(xs: number[]): number {
  if (xs.length === 0) {
    throw new Error("modeValue: input array is empty");
  }

  const counts = new Map<number, number>();
  for (const x of xs) {
    counts.set(x, (counts.get(x) ?? 0) + 1);
  }

  let best = 0;
  let bestCount = -1;
  for (const [value, count] of counts) {
    if (count > bestCount || (count === bestCount && value < best)) {
      best = value;
      bestCount = count;
    }
  }

  return best;
}
