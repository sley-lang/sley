pub fn inversions(xs: &[i64]) -> i64 {
    fn sort_count(v: &mut [i64], buf: &mut Vec<i64>) -> i64 {
        let n = v.len();
        if n < 2 {
            return 0;
        }
        let mid = n / 2;
        let mut count = {
            let (left, right) = v.split_at_mut(mid);
            sort_count(left, buf) + sort_count(right, buf)
        };

        buf.clear();
        let (mut i, mut j) = (0, mid);
        while i < mid && j < n {
            if v[i] <= v[j] {
                buf.push(v[i]);
                i += 1;
            } else {
                // v[j] is smaller than all remaining left elements (v[i..mid])
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
    sort_count(&mut v, &mut buf)
}
