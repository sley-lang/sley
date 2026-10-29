export function inversions(xs: number[]): number {
  const n = xs.length;
  if (n < 2) {
    return 0;
  }

  const buf: number[] = new Array(n);
  let count = 0;

  // Bottom-up merge sort; counts pairs where a later element is strictly smaller.
  let src = xs.slice();
  let width = 1;
  while (width < n) {
    for (let lo = 0; lo < n; lo += 2 * width) {
      const mid = Math.min(lo + width, n);
      const hi = Math.min(lo + 2 * width, n);
      let i = lo;
      let j = mid;
      let k = lo;
      while (i < mid && j < hi) {
        if (src[j] < src[i]) {
          // src[j] is smaller than every remaining left element (left half is sorted).
          count += mid - i;
          buf[k++] = src[j++];
        } else {
          buf[k++] = src[i++];
        }
      }
      while (i < mid) {
        buf[k++] = src[i++];
      }
      while (j < hi) {
        buf[k++] = src[j++];
      }
    }
    const tmp = src;
    src = buf.slice(0, n);
    buf.length = n;
    void tmp;
    width *= 2;
  }

  return count;
}
