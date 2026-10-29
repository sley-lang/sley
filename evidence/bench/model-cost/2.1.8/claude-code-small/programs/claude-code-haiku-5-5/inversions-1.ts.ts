export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) {
    return 0;
  }

  // Merge sort counting: count pairs i < j with xs[i] > xs[j].
  const arr = xs.slice();
  const buf = new Array<number>(n);

  const sortCount = (lo: number, hi: number): number => {
    // half-open range [lo, hi)
    if (hi - lo < 2) {
      return 0;
    }
    const mid = (lo + hi) >>> 1;
    let count = sortCount(lo, mid) + sortCount(mid, hi);

    let i = lo;
    let j = mid;
    let k = lo;
    while (i < mid && j < hi) {
      if (arr[i] <= arr[j]) {
        buf[k++] = arr[i++];
      } else {
        // arr[i..mid) are all greater than arr[j]
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
    return count;
  };

  return sortCount(0, n);
}
