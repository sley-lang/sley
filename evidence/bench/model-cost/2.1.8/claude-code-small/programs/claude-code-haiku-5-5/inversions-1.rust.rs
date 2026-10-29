pub fn inversions(xs: &[i64]) -> i64 {
    fn sort_count(v: &mut [i64], buf: &mut Vec<i64>) -> i64 {
        let n = v.len();
        if n <= 1 {
            return 0;
        }
        let mid = n / 2;
        let (left, right) = v.split_at_mut(mid);
        let mut count = sort_count(left, buf) + sort_count(right, buf);

        buf.clear();
        let (mut i, mut j) = (0usize, 0usize);
        while i < left.len() && j < right.len() {
            if left[i] <= right[j] {
                buf.push(left[i]);
                i += 1;
            } else {
                // right[j] is strictly smaller than every remaining left element
                count += (left.len() - i) as i64;
                buf.push(right[j]);
                j += 1;
            }
        }
        buf.extend_from_slice(&left[i..]);
        buf.extend_from_slice(&right[j..]);
        v.copy_from_slice(buf);
        count
    }

    let mut v = xs.to_vec();
    let mut buf = Vec::with_capacity(xs.len());
    sort_count(&mut v, &mut buf)
}
