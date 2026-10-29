pub fn inversions(xs: &[i64]) -> i64 {
    let mut a = xs.to_vec();
    let mut tmp: Vec<i64> = Vec::with_capacity(a.len());
    sort_count(&mut a, &mut tmp)
}

fn sort_count(a: &mut [i64], tmp: &mut Vec<i64>) -> i64 {
    let n = a.len();
    if n < 2 {
        return 0;
    }
    let mid = n / 2;
    let (left, right) = a.split_at_mut(mid);
    let mut total = sort_count(left, tmp) + sort_count(right, tmp);

    tmp.clear();
    let (mut i, mut j) = (0usize, 0usize);
    while i < left.len() && j < right.len() {
        if left[i] <= right[j] {
            tmp.push(left[i]);
            i += 1;
        } else {
            // right[j] is smaller than every remaining element of left
            total += (left.len() - i) as i64;
            tmp.push(right[j]);
            j += 1;
        }
    }
    tmp.extend_from_slice(&left[i..]);
    tmp.extend_from_slice(&right[j..]);
    a.copy_from_slice(tmp);
    total
}
