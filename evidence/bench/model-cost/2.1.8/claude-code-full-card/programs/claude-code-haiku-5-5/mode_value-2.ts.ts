export function modeValue(xs: number[]): number {
  if (xs.length === 0) {
    throw new Error("modeValue: empty input");
  }

  const counts = new Map<number, number>();
  for (const x of xs) {
    counts.set(x, (counts.get(x) ?? 0) + 1);
  }

  let best = 0;
  let bestValue = Infinity;
  for (const [value, count] of counts) {
    if (count > best || (count === best && value < bestValue)) {
      best = count;
      bestValue = value;
    }
  }

  return bestValue;
}
