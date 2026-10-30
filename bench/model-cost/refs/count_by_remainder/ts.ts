export function countByRemainder(xs: number[], m: number): number[] {
  if (m < 1 || m > 200) {
    throw new Error("m must be within [1, 200]");
  }
  const counts = new Array<number>(m).fill(0);
  for (const x of xs) {
    counts[((x % m) + m) % m]++;
  }
  return counts;
}
