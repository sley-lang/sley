export function secondLargest(xs: number[]): number {
  let max: number | undefined;
  let second: number | undefined;

  for (const x of xs) {
    if (Number.isNaN(x)) {
      throw new Error("secondLargest: NaN is not a valid value");
    }
    if (max === undefined || x > max) {
      if (max !== undefined) {
        second = max;
      }
      max = x;
    } else if (x < max && (second === undefined || x > second)) {
      second = x;
    }
  }

  if (second === undefined) {
    throw new Error("secondLargest: fewer than two distinct values");
  }
  return second;
}
