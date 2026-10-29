export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) {
    return 0;
  }

  const buf = new Array<number>(n);

  // Bottom-up merge sort counting cross-half inversions.
  // Ties (equal values) are not counted since we only count strict xs[i] > xs[j].
  function sortCount(arr: number[], tmp: number[], lo: number, hi: number): number {
    if (hi - lo < 2) {
      return 0;
    }
    const mid = (lo + hi) >>> 1;
    let count = sortCount(arr, tmp, lo, mid) + sortCount(arr, tmp, mid, hi);

    let i = lo;
    let j = mid;
    let k = lo;
    while (i < mid && j < hi) {
      if (arr[i] <= arr[j]) {
        tmp[k++] = arr[i++];
      } else {
        // arr[i..mid) are all greater than arr[j]
        count += mid - i;
        tmp[k++] = arr[j++];
      }
    }
    while (i < mid) {
      tmp[k++] = arr[i++];
    }
    while (j < hi) {
      tmp[k++] = arr[j++];
    }
    for (let p = lo; p < hi; p++) {
      arr[p] = tmp[p];
    }
    return count;
  }

  const copy = xs.slice();
  return sortCount(copy, buf, 0, n);
}
