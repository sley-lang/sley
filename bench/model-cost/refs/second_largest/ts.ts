export function secondLargest(xs: number[]): number {
  const distinct = [...new Set(xs)].sort((a, b) => b - a);
  if (distinct.length < 2) throw new Error("too few values");
  return distinct[1];
}
