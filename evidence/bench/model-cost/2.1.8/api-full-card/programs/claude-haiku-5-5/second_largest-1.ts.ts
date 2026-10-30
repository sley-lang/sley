export function secondLargest(xs: number[]): number {
  if (!Array.isArray(xs)) {
    throw new Error("secondLargest: input must be an array");
  }
  let max: number | undefined = undefined;
  let second: number | undefined = undefined;
  for (const x of xs) {
    if (typeof x !== "number" || Number.isNaN(x)) {
      throw new Error("secondLargest: all elements must be non-NaN numbers");
    }
    if (max === undefined || x > max) {
      if (max !== undefined && x !== max) {
        second = max;
      }
      max = x;
    } else if (x !== max && (second === undefined || x > second)) {
      second = x;
    }
  }
  if (max === undefined || second === undefined) {
    throw new Error("secondLargest: fewer than two distinct values");
  }
  return second;
}
