//! Document-identity minting: 128-bit ULID encoded as 26 Crockford base-32 chars.
//! Deterministic under injected Clock + Rng for testability.

use crate::adapter::{Clock, Rng};
use crate::error::SessionError;
use std::time::UNIX_EPOCH;

/// Crockford base-32 alphabet (excludes I, L, O, U). 32 symbols.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encode a 128-bit value as the canonical 26-character ULID string
/// (big-endian, top character carries the high 2 padding bits so it is 0-7).
fn encode_crockford(val: u128) -> String {
    let mut out = [0u8; 26];
    for (i, slot) in out.iter_mut().enumerate() {
        let idx = ((val >> (5 * (25 - i))) & 0x1f) as usize;
        // idx is always < 32, so indexing CROCKFORD is in-bounds; use get to avoid any panic path.
        *slot = match CROCKFORD.get(idx) {
            Some(c) => *c,
            None => b'0',
        };
    }
    // out is ASCII by construction.
    String::from_utf8(out.into()).unwrap_or_default()
}

/// Mint a fresh ULID document id: 48-bit millisecond timestamp (from `clock`)
/// in the high bits, 80 bits of randomness (from `rng`) in the low bits.
pub fn mint_ulid(clock: &impl Clock, rng: &impl Rng) -> Result<String, SessionError> {
    let millis = clock
        .now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| SessionError::new(format!("system clock is before the unix epoch: {e}")))?
        .as_millis();
    let time48 = millis & 0xFFFF_FFFF_FFFF;

    let mut rand_bytes = [0u8; 10]; // 80 bits
    rng.fill_bytes(&mut rand_bytes)?;
    let mut rand80: u128 = 0;
    for b in rand_bytes {
        rand80 = (rand80 << 8) | u128::from(b);
    }

    let val = (time48 << 80) | rand80;
    Ok(encode_crockford(val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{FakeClock, FakeRng};
    use std::time::{Duration, UNIX_EPOCH};

    fn crockford_chars() -> &'static [u8] {
        CROCKFORD
    }

    #[test]
    fn mints_26_char_string() {
        let clock = FakeClock(UNIX_EPOCH + Duration::from_millis(1));
        let rng = FakeRng(0);
        let id = mint_ulid(&clock, &rng).unwrap();
        assert_eq!(id.len(), 26);
        for ch in id.bytes() {
            assert!(crockford_chars().contains(&ch), "unexpected char: {ch}");
        }
    }

    #[test]
    fn is_deterministic_under_fakes() {
        let clock = FakeClock(UNIX_EPOCH + Duration::from_millis(1));
        let rng = FakeRng(0);
        let first = mint_ulid(&clock, &rng).unwrap();
        let second = mint_ulid(&clock, &rng).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn different_time_changes_prefix() {
        let clock1 = FakeClock(UNIX_EPOCH + Duration::from_millis(1000));
        let clock2 = FakeClock(UNIX_EPOCH + Duration::from_millis(2000));
        let rng = FakeRng(0x00);
        let id1 = mint_ulid(&clock1, &rng).unwrap();
        let id2 = mint_ulid(&clock2, &rng).unwrap();
        // First 10 chars encode the timestamp — they must differ.
        assert_ne!(&id1[..10], &id2[..10]);
        // Last 16 chars encode the randomness — FakeRng is identical, so they must match.
        assert_eq!(&id1[10..], &id2[10..]);
    }

    #[test]
    fn different_rng_changes_suffix() {
        let clock = FakeClock(UNIX_EPOCH + Duration::from_millis(1000));
        let rng0 = FakeRng(0x00);
        let rng1 = FakeRng(0xFF);
        let id0 = mint_ulid(&clock, &rng0).unwrap();
        let id1 = mint_ulid(&clock, &rng1).unwrap();
        // First 10 chars encode the timestamp — same clock, so they must match.
        assert_eq!(&id0[..10], &id1[..10]);
        // Last 16 chars encode the randomness — different RNGs, so they must differ.
        assert_ne!(&id0[10..], &id1[10..]);
    }

    #[test]
    fn known_vector() {
        // FakeClock at UNIX_EPOCH exactly → millis = 0.
        // FakeRng(0x00) → all rand bytes are 0.
        // val = 0 → all 26 Crockford chars are '0'.
        let clock = FakeClock(UNIX_EPOCH);
        let rng = FakeRng(0x00);
        let id = mint_ulid(&clock, &rng).unwrap();
        assert_eq!(id, "00000000000000000000000000");
    }

    #[test]
    fn clock_before_epoch_errors() {
        if let Some(before_epoch) = UNIX_EPOCH.checked_sub(Duration::from_secs(1)) {
            let clock = FakeClock(before_epoch);
            let rng = FakeRng(0x00);
            assert!(mint_ulid(&clock, &rng).is_err());
        }
        // If checked_sub returns None on this platform, the assertion is skipped gracefully.
    }
}
