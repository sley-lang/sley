export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) return 0;

  const arr = xs.slice();
  const buf = new Array<number>(n);

  // Bottom-up merge sort counting cross inversions.
  let count = 0;
  for (let width = 1; width < n; width *= 2) {
    for (let lo = 0; lo < n; lo += 2 * width) {
      const mid = Math.min(lo + width, n);
      const hi = Math.min(lo + 2 * width, n);
      let i = lo;
      let j = mid;
      let k = lo;
      while (i < mid && j < hi) {
        if (arr[i] <= arr[j]) {
          buf[k++] = arr[i++];
        } else {
          // arr[j] is strictly smaller than all remaining left elements
          count += mid - i;
          buf[k++] = arr[j++];
        }
      }
      while (i < mid) buf[k++] = arr[i++];
      while (j < hi) buf[k++] = arr[j++];
    }
    for (let t = 0; t < n; t++) arr[t] = buf[t];
  }

  return count;
}
