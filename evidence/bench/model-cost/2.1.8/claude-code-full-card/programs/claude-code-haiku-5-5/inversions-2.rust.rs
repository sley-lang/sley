/// Returns the number of index pairs `i < j` with `xs[i] > xs[j]`.
///
/// Uses a merge sort that counts cross-inversions during each merge,
/// running in O(n log n) time. The input slice is not modified.
pub fn inversions(xs: &[i64]) -> i64 {
    let mut v = xs.to_vec();
    let mut tmp: Vec<i64> = Vec::with_capacity(xs.len());
    count_and_sort(&mut v, &mut tmp)
}

/// Sorts `xs` in place and returns the number of inversions within it.
fn count_and_sort(xs: &mut [i64], tmp: &mut Vec<i64>) -> i64 {
    let n = xs.len();
    if n < 2 {
        return 0;
    }

    let mid = n / 2;
    let mut inv = count_and_sort(&mut xs[..mid], tmp) + count_and_sort(&mut xs[mid..], tmp);

    tmp.clear();
    let (mut i, mut j) = (0usize, mid);
    while i < mid && j < n {
        if xs[i] <= xs[j] {
            tmp.push(xs[i]);
            i += 1;
        } else {
            // xs[j] is smaller than every remaining element of the left half.
            inv += (mid - i) as i64;
            tmp.push(xs[j]);
            j += 1;
        }
    }
    tmp.extend_from_slice(&xs[i..mid]);
    tmp.extend_from_slice(&xs[j..n]);
    xs.copy_from_slice(tmp);

    inv
}
