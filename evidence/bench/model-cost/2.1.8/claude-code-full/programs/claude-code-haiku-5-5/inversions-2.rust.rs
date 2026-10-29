pub fn inversions(xs: &[i64]) -> i64 {
    fn merge_count(v: &mut [i64], buf: &mut Vec<i64>) -> i64 {
        let n = v.len();
        if n <= 1 {
            return 0;
        }
        let mid = n / 2;
        let mut count = {
            let (left, right) = v.split_at_mut(mid);
            merge_count(left, buf) + merge_count(right, buf)
        };
        buf.clear();
        let (mut i, mut j) = (0, mid);
        while i < mid && j < n {
            if v[i] <= v[j] {
                buf.push(v[i]);
                i += 1;
            } else {
                // v[i..mid] are all greater than v[j]
                count += (mid - i) as i64;
                buf.push(v[j]);
                j += 1;
            }
        }
        buf.extend_from_slice(&v[i..mid]);
        buf.extend_from_slice(&v[j..n]);
        v.copy_from_slice(buf);
        count
    }

    let mut v = xs.to_vec();
    let mut buf = Vec::with_capacity(xs.len());
    merge_count(&mut v, &mut buf)
}
