export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) return 0;
  const a = xs.slice();
  const buf = new Array<number>(n);
  let count = 0;

  const sort = (lo: number, hi: number): void => {
    if (hi - lo < 2) return;
    const mid = (lo + hi) >>> 1;
    sort(lo, mid);
    sort(mid, hi);
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
  };

  sort(0, n);
  return count;
}
