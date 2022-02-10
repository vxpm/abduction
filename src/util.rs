use std::ops::{Div, Rem};

pub fn boxed_array<T, const N: usize>(value: T) -> Box<[T; N]>
where
    T: Clone,
{
    let v: Vec<_> = (0..N).map(|_| value.clone()).collect();
    let b = v.into_boxed_slice();

    Box::<[T; N]>::try_from(b).ok().unwrap()
}

pub fn read_bytes<P>(path: P) -> anyhow::Result<Vec<u8>>
where
    P: AsRef<std::path::Path>,
{
    let mut file = std::fs::File::open(path.as_ref())?;
    let mut buffer = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut buffer)?;

    Ok(buffer)
}

#[inline]
pub fn div_rem<A, B>(a: A, b: B) -> (<A as Div<B>>::Output, <A as Rem<B>>::Output)
where
    A: Div<B> + Rem<B> + Copy,
    B: Copy,
{
    (a / b, a % b)
}
