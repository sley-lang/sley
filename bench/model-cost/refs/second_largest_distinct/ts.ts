export function secondLargestDistinct(xs: number[]): number {
  if (xs.length === 0) {
    throw new Error("fewer than two distinct values");
  }
  const max = Math.max(...xs);
  let second: number | null = null;
  for (const x of xs) {
    if (x < max && (second === null || x > second)) {
      second = x;
    }
  }
  if (second === null) {
    throw new Error("fewer than two distinct values");
  }
  return second;
}
