// Go sort.Sort's PDQsort, translated to preserve random tie comparisons.
// Copyright 2022 The Go Authors. BSD license: third_party/GO_LICENSE.
pub fn sort<T: Copy, F: FnMut(T, T) -> bool>(v: &mut [T], less: &mut F) {
    if v.len() > 1 {
        let limit = (usize::BITS - v.len().leading_zeros()) as usize;
        pdq(v, 0, v.len(), limit, less);
    }
}
fn insertion<T: Copy, F: FnMut(T, T) -> bool>(v: &mut [T], a: usize, b: usize, l: &mut F) {
    for i in a + 1..b {
        let mut j = i;
        while j > a && l(v[j], v[j - 1]) {
            v.swap(j, j - 1);
            j -= 1;
        }
    }
}
fn sift<T: Copy, F: FnMut(T, T) -> bool>(
    v: &mut [T],
    mut root: usize,
    hi: usize,
    first: usize,
    l: &mut F,
) {
    loop {
        let mut child = 2 * root + 1;
        if child >= hi {
            break;
        }
        if child + 1 < hi && l(v[first + child], v[first + child + 1]) {
            child += 1;
        }
        if !l(v[first + root], v[first + child]) {
            return;
        }
        v.swap(first + root, first + child);
        root = child;
    }
}
fn heap<T: Copy, F: FnMut(T, T) -> bool>(v: &mut [T], a: usize, b: usize, l: &mut F) {
    let hi = b - a;
    for i in (0..=(hi - 1) / 2).rev() {
        sift(v, i, hi, a, l);
    }
    for i in (0..hi).rev() {
        v.swap(a, a + i);
        sift(v, 0, i, a, l);
    }
}
fn median<T: Copy, F: FnMut(T, T) -> bool>(
    v: &[T],
    mut a: usize,
    mut b: usize,
    mut c: usize,
    s: &mut usize,
    l: &mut F,
) -> usize {
    if l(v[b], v[a]) {
        std::mem::swap(&mut a, &mut b);
        *s += 1;
    }
    if l(v[c], v[b]) {
        std::mem::swap(&mut b, &mut c);
        *s += 1;
    }
    if l(v[b], v[a]) {
        std::mem::swap(&mut a, &mut b);
        *s += 1;
    }
    b
}
fn pivot<T: Copy, F: FnMut(T, T) -> bool>(
    v: &[T],
    a: usize,
    b: usize,
    l: &mut F,
) -> (usize, usize) {
    let n = b - a;
    let (mut i, mut j, mut k) = (a + n / 4, a + n / 4 * 2, a + n / 4 * 3);
    let mut s = 0;
    if n >= 8 {
        if n >= 50 {
            i = median(v, i - 1, i, i + 1, &mut s, l);
            j = median(v, j - 1, j, j + 1, &mut s, l);
            k = median(v, k - 1, k, k + 1, &mut s, l);
        }
        j = median(v, i, j, k, &mut s, l);
    }
    (
        j,
        if s == 0 {
            1
        } else if s == 12 {
            2
        } else {
            0
        },
    )
}
fn partial<T: Copy, F: FnMut(T, T) -> bool>(v: &mut [T], a: usize, b: usize, l: &mut F) -> bool {
    let mut i = a + 1;
    for _ in 0..5 {
        while i < b && !l(v[i], v[i - 1]) {
            i += 1;
        }
        if i == b {
            return true;
        }
        if b - a < 50 {
            return false;
        }
        v.swap(i, i - 1);
        if i - a >= 2 {
            for j in (1..i).rev() {
                if !l(v[j], v[j - 1]) {
                    break;
                }
                v.swap(j, j - 1);
            }
        }
        if b - i >= 2 {
            for j in i + 1..b {
                if !l(v[j], v[j - 1]) {
                    break;
                }
                v.swap(j, j - 1);
            }
        }
    }
    false
}
fn partition<T: Copy, F: FnMut(T, T) -> bool>(
    v: &mut [T],
    a: usize,
    b: usize,
    p: usize,
    l: &mut F,
) -> (usize, bool) {
    v.swap(a, p);
    let mut i = a + 1;
    let mut j = b - 1;
    while i <= j && l(v[i], v[a]) {
        i += 1;
    }
    while i <= j && !l(v[j], v[a]) {
        j -= 1;
    }
    if i > j {
        v.swap(j, a);
        return (j, true);
    }
    v.swap(i, j);
    i += 1;
    j -= 1;
    loop {
        while i <= j && l(v[i], v[a]) {
            i += 1;
        }
        while i <= j && !l(v[j], v[a]) {
            j -= 1;
        }
        if i > j {
            break;
        }
        v.swap(i, j);
        i += 1;
        j -= 1;
    }
    v.swap(j, a);
    (j, false)
}
fn equal<T: Copy, F: FnMut(T, T) -> bool>(
    v: &mut [T],
    a: usize,
    b: usize,
    p: usize,
    l: &mut F,
) -> usize {
    v.swap(a, p);
    let mut i = a + 1;
    let mut j = b - 1;
    loop {
        while i <= j && !l(v[a], v[i]) {
            i += 1;
        }
        while i <= j && l(v[a], v[j]) {
            j -= 1;
        }
        if i > j {
            break;
        }
        v.swap(i, j);
        i += 1;
        j -= 1;
    }
    i
}
fn patterns<T>(v: &mut [T], a: usize, b: usize) {
    let n = b - a;
    if n >= 8 {
        let mut r = n as u64;
        let modulus = 1usize << (usize::BITS - n.leading_zeros());
        for idx in a + n / 4 * 2 - 1..=a + n / 4 * 2 + 1 {
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            let mut other = r as usize & (modulus - 1);
            if other >= n {
                other -= n;
            }
            v.swap(idx, a + other);
        }
    }
}
fn pdq<T: Copy, F: FnMut(T, T) -> bool>(
    v: &mut [T],
    mut a: usize,
    mut b: usize,
    mut limit: usize,
    l: &mut F,
) {
    let (mut balanced, mut partitioned) = (true, true);
    loop {
        let n = b - a;
        if n <= 12 {
            insertion(v, a, b, l);
            return;
        }
        if limit == 0 {
            heap(v, a, b, l);
            return;
        }
        if !balanced {
            patterns(v, a, b);
            limit -= 1;
        }
        let (mut p, mut hint) = pivot(v, a, b, l);
        if hint == 2 {
            v[a..b].reverse();
            p = b - 1 - (p - a);
            hint = 1;
        }
        if balanced && partitioned && hint == 1 && partial(v, a, b, l) {
            return;
        }
        if a > 0 && !l(v[a - 1], v[p]) {
            a = equal(v, a, b, p, l);
            continue;
        }
        let (mid, already) = partition(v, a, b, p, l);
        partitioned = already;
        let (left, right) = (mid - a, b - mid);
        if left < right {
            balanced = left >= n / 8;
            pdq(v, a, mid, limit, l);
            a = mid + 1;
        } else {
            balanced = right >= n / 8;
            pdq(v, mid + 1, b, limit, l);
            b = mid;
        }
    }
}
