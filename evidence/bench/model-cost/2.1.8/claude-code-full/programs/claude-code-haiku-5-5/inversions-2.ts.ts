export function inversions(xs: number[]): number {
  const a = xs.slice();
  const buf = new Array<number>(a.length);

  function sortCount(lo: number, hi: number): number {
    if (hi - lo <= 1) return 0;
    const mid = (lo + hi) >> 1;
    let count = sortCount(lo, mid) + sortCount(mid, hi);

    let i = lo;
    let j = mid;
    let k = lo;
    while (i < mid && j < hi) {
      if (a[i] <= a[j]) {
        buf[k++] = a[i++];
      } else {
        // a[i] > a[j], and every remaining element in the left half is also > a[j]
        count += mid - i;
        buf[k++] = a[j++];
      }
    }
    while (i < mid) buf[k++] = a[i++];
    while (j < hi) buf[k++] = a[j++];
    for (let t = lo; t < hi; t++) a[t] = buf[t];

    return count;
  }

  return sortCount(0, a.length);
}
