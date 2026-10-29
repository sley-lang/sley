export function secondLargest(xs: number[]): number {
  if (!Array.isArray(xs)) {
    throw new Error("secondLargest: input must be an array");
  }

  let largest: number | undefined;
  let second: number | undefined;

  for (const x of xs) {
    if (typeof x !== "number" || Number.isNaN(x)) {
      throw new Error("secondLargest: input must contain only numbers");
    }
    if (largest === undefined || x > largest) {
      if (largest !== undefined && x !== largest) {
        second = largest;
      }
      largest = x;
    } else if (x < largest && (second === undefined || x > second)) {
      second = x;
    }
  }

  if (largest === undefined || second === undefined) {
    throw new Error("secondLargest: fewer than two distinct values");
  }

  return second;
}
