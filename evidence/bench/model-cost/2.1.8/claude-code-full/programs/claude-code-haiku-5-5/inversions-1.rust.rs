pub fn inversions(xs: &[i64]) -> i64 {
    let mut v = xs.to_vec();
    let mut buf = vec![0i64; v.len()];
    sort_count(&mut v, &mut buf)
}

fn sort_count(v: &mut [i64], buf: &mut [i64]) -> i64 {
    let n = v.len();
    if n < 2 {
        return 0;
    }
    let mid = n / 2;
    let mut count = {
        let (left, right) = v.split_at_mut(mid);
        let (lbuf, rbuf) = buf.split_at_mut(mid);
        sort_count(left, lbuf) + sort_count(right, rbuf)
    };

    let (mut i, mut j, mut k) = (0usize, mid, 0usize);
    while i < mid && j < n {
        if v[i] <= v[j] {
            buf[k] = v[i];
            i += 1;
        } else {
            buf[k] = v[j];
            j += 1;
            count += (mid - i) as i64;
        }
        k += 1;
    }
    while i < mid {
        buf[k] = v[i];
        i += 1;
        k += 1;
    }
    while j < n {
        buf[k] = v[j];
        j += 1;
        k += 1;
    }
    v.copy_from_slice(&buf[..n]);
    count
}
