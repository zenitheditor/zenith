//! Random-number-generator adapter trait, OS-backed implementation, and deterministic test fake.

use crate::error::SessionError;

/// Abstraction over a random-byte source.
///
/// Callers receive `&impl Rng` so that production code can use [`OsRng`] while
/// tests substitute [`FakeRng`] for full determinism.
///
/// The trait is fallible (`Result`) because OS entropy can fail; library code
/// must not panic on entropy exhaustion or OS error.
pub trait Rng {
    /// Fill `buf` with random (or deterministic, in fakes) bytes.
    fn fill_bytes(&self, buf: &mut [u8]) -> Result<(), SessionError>;
}

/// Deterministic test RNG: fills every byte of `buf` with `self.0`.
///
/// Useful for asserting exact byte sequences in unit tests without needing a
/// real entropy source.
pub struct FakeRng(pub u8);

impl Rng for FakeRng {
    fn fill_bytes(&self, buf: &mut [u8]) -> Result<(), SessionError> {
        buf.fill(self.0);
        Ok(())
    }
}

/// OS-backed entropy source (via `getrandom`).
pub struct OsRng;

impl Rng for OsRng {
    fn fill_bytes(&self, buf: &mut [u8]) -> Result<(), SessionError> {
        getrandom::fill(buf).map_err(|e| SessionError::new(format!("os entropy failure: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_rng_fills_all_bytes_with_its_value() {
        let rng = FakeRng(0xAB);
        let mut buf = [0u8; 8];
        rng.fill_bytes(&mut buf).unwrap();
        assert_eq!(buf, [0xAB; 8]);
    }

    #[test]
    fn fake_rng_zero_value() {
        let rng = FakeRng(0x00);
        let mut buf = [0xFFu8; 4];
        rng.fill_bytes(&mut buf).unwrap();
        assert_eq!(buf, [0x00; 4]);
    }

    #[test]
    fn os_rng_smoke() {
        let result = OsRng.fill_bytes(&mut [0u8; 16]);
        assert!(result.is_ok());
    }
}
