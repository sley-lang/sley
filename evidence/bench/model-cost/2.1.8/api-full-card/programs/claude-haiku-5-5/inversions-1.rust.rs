pub fn inversions(xs: &[i64]) -> i64 {
    fn merge_count(v: &mut Vec<i64>, buf: &mut Vec<i64>, lo: usize, hi: usize) -> i64 {
        if hi - lo <= 1 {
            return 0;
        }
        let mid = lo + (hi - lo) / 2;
        let mut count = merge_count(v, buf, lo, mid) + merge_count(v, buf, mid, hi);
        let (mut i, mut j, mut k) = (lo, mid, lo);
        while i < mid && j < hi {
            if v[i] <= v[j] {
                buf[k] = v[i];
                i += 1;
            } else {
                buf[k] = v[j];
                count += (mid - i) as i64;
                j += 1;
            }
            k += 1;
        }
        while i < mid {
            buf[k] = v[i];
            i += 1;
            k += 1;
        }
        while j < hi {
            buf[k] = v[j];
            j += 1;
            k += 1;
        }
        v[lo..hi].copy_from_slice(&buf[lo..hi]);
        count
    }

    let n = xs.len();
    let mut v: Vec<i64> = xs.to_vec();
    let mut buf: Vec<i64> = vec![0; n];
    merge_count(&mut v, &mut buf, 0, n)
}
