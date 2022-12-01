use std::io::{BufReader, Read};

pub trait ReadableFromBytes<const N: usize> {
    fn read(bytes: [u8; N]) -> Self;
}

macro_rules! impl_readable {
  ($($type:ty),+) => {
      $(
          impl ReadableFromBytes<{ std::mem::size_of::<$type>() }> for $type {
              fn read(bytes: [u8; std::mem::size_of::<$type>()]) -> Self {
                  <$type>::from_le_bytes(bytes)
              }
          }
      )+
  };
}

impl_readable! { i16, u16, u32, u64, f32 }

pub fn read_value<'a, const N: usize, R: Read, T: ReadableFromBytes<N>>(
    reader: &mut BufReader<R>,
    into: &mut T,
    error_msg: &'a str,
) -> Result<(), &'a str> {
    let mut buffer = [0; N];
    reader.read_exact(&mut buffer).map_err(|_| error_msg)?;

    *into = T::read(buffer);
    Ok(())
}
