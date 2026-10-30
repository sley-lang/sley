pub fn dense_rank(xs: &[i64]) -> Vec<i64> {
    let mut distinct: Vec<i64> = xs.to_vec();
    distinct.sort_unstable_by(|a, b| b.cmp(a));
    distinct.dedup();
    xs.iter()
        .map(|x| {
            // distinct is sorted descending; rank is 1 + index of x.
            let idx = distinct.binary_search_by(|probe| x.cmp(probe)).unwrap_or(0);
            idx as i64 + 1
        })
        .collect()
}
