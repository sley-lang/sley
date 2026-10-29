export function secondLargest(xs: number[]): number {
  let max: number | undefined = undefined;
  let second: number | undefined = undefined;

  for (const x of xs) {
    if (max === undefined || x > max) {
      if (max !== undefined && x !== max) {
        second = max;
      }
      max = x;
    } else if (x < max) {
      if (second === undefined || x > second) {
        second = x;
      }
    }
  }

  if (max === undefined || second === undefined) {
    throw new Error("secondLargest: fewer than two distinct values");
  }
  return second;
}
