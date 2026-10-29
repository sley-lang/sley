export function secondLargest(xs: number[]): number {
  if (xs.some((x) => Number.isNaN(x))) {
    throw new Error("secondLargest: input contains NaN");
  }
  const distinct = Array.from(new Set(xs));
  if (distinct.length < 2) {
    throw new Error("secondLargest: need at least two distinct values");
  }
  distinct.sort((a, b) => b - a);
  return distinct[1];
}
