export function secondLargest(xs: number[]): number {
  const distinct = Array.from(new Set(xs));
  if (distinct.length < 2) {
    throw new Error("secondLargest: fewer than two distinct values");
  }
  let largest = -Infinity;
  let second = -Infinity;
  for (const x of distinct) {
    if (x > largest) {
      second = largest;
      largest = x;
    } else if (x > second) {
      second = x;
    }
  }
  return second;
}
