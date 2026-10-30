export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) return 0;
  const a = xs.slice();
  const buf = new Array<number>(n);

  function sortCount(lo: number, hi: number): number {
    // sorts a[lo..hi) and returns inversion count within that range
    if (hi - lo < 2) return 0;
    const mid = (lo + hi) >>> 1;
    let count = sortCount(lo, mid) + sortCount(mid, hi);
    let i = lo;
    let j = mid;
    let k = lo;
    while (i < mid && j < hi) {
      if (a[i] <= a[j]) {
        buf[k++] = a[i++];
      } else {
        count += mid - i;
        buf[k++] = a[j++];
      }
    }
    while (i < mid) buf[k++] = a[i++];
    while (j < hi) buf[k++] = a[j++];
    for (let t = lo; t < hi; t++) a[t] = buf[t];
    return count;
  }

  return sortCount(0, n);
}
