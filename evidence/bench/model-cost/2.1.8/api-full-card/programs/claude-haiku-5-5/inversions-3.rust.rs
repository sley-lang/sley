pub fn inversions(xs: &[i64]) -> i64 {
    fn sort_count(v: &mut Vec<i64>) -> i64 {
        let n = v.len();
        if n <= 1 {
            return 0;
        }
        let mid = n / 2;
        let mut left = v[..mid].to_vec();
        let mut right = v[mid..].to_vec();
        let mut count = sort_count(&mut left) + sort_count(&mut right);

        let mut i = 0;
        let mut j = 0;
        let mut k = 0;
        while i < left.len() && j < right.len() {
            if left[i] <= right[j] {
                v[k] = left[i];
                i += 1;
            } else {
                v[k] = right[j];
                j += 1;
                count += (left.len() - i) as i64;
            }
            k += 1;
        }
        while i < left.len() {
            v[k] = left[i];
            i += 1;
            k += 1;
        }
        while j < right.len() {
            v[k] = right[j];
            j += 1;
            k += 1;
        }
        count
    }

    let mut v = xs.to_vec();
    sort_count(&mut v)
}
