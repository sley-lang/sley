export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) {
    return 0;
  }

  // Merge sort counting: count pairs (i < j) with xs[i] > xs[j].
  const arr = xs.slice();
  const buf: number[] = new Array(n);
  let count = 0;

  for (let width = 1; width < n; width *= 2) {
    for (let lo = 0; lo < n - width; lo += 2 * width) {
      const mid = lo + width;
      const hi = Math.min(lo + 2 * width, n);
      let i = lo;
      let j = mid;
      let k = lo;
      while (i < mid && j < hi) {
        if (arr[i] <= arr[j]) {
          buf[k++] = arr[i++];
        } else {
          // arr[j] is smaller than all remaining left-half elements.
          count += mid - i;
          buf[k++] = arr[j++];
        }
      }
      while (i < mid) {
        buf[k++] = arr[i++];
      }
      while (j < hi) {
        buf[k++] = arr[j++];
      }
      for (let t = lo; t < hi; t++) {
        arr[t] = buf[t];
      }
    }
  }

  return count;
}
