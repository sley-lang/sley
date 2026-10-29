export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) return 0;

  const buf = xs.slice();
  const tmp = new Array<number>(n);
  let count = 0;

  // Bottom-up merge sort; counts cross-run inversions when taking from the right run.
  for (let width = 1; width < n; width *= 2) {
    for (let lo = 0; lo < n - width; lo += 2 * width) {
      const mid = lo + width;
      const hi = Math.min(lo + 2 * width, n);
      let i = lo;
      let j = mid;
      let k = lo;
      while (i < mid && j < hi) {
        if (buf[j] < buf[i]) {
          // buf[j] is smaller than every remaining left element: each forms an inversion.
          count += mid - i;
          tmp[k++] = buf[j++];
        } else {
          tmp[k++] = buf[i++];
        }
      }
      while (i < mid) tmp[k++] = buf[i++];
      while (j < hi) tmp[k++] = buf[j++];
      for (let t = lo; t < hi; t++) buf[t] = tmp[t];
    }
  }

  return count;
}
