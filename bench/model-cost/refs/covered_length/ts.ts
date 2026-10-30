export function coveredLength(xs: number[]): number {
  if (xs.length % 2 !== 0) {
    throw new Error("odd number of elements");
  }
  const intervals: Array<[number, number]> = [];
  for (let i = 0; i < xs.length; i += 2) {
    if (xs[i] > xs[i + 1]) {
      throw new Error("start greater than end");
    }
    intervals.push([xs[i], xs[i + 1]]);
  }
  intervals.sort((a, b) => a[0] - b[0]);
  let total = 0;
  let curStart = 0;
  let curEnd = 0;
  let open = false;
  for (const [start, end] of intervals) {
    if (!open) {
      curStart = start;
      curEnd = end;
      open = true;
    } else if (start <= curEnd) {
      curEnd = Math.max(curEnd, end);
    } else {
      total += curEnd - curStart;
      curStart = start;
      curEnd = end;
    }
  }
  if (open) {
    total += curEnd - curStart;
  }
  return total;
}
