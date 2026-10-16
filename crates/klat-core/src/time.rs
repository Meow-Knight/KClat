//! Time as the rules see it. klat-core never reads the clock (clippy.toml): the app reads it once per command
//! and passes a [`UnixMs`] in, so tests choose the time and never wait for it.
//!
//! Two types so a moment and a length cannot be swapped: `expire(now, stale_after)` with the arguments the
//! wrong way round does not compile.

use serde::{Deserialize, Serialize};

/// A moment, in milliseconds since the Unix epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct UnixMs(pub u64);

/// A length of time, in milliseconds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "number"))]
pub struct DurationMs(pub u64);

impl UnixMs {
    /// How long after `earlier` this is; zero if it is not after it (the clock went back).
    pub const fn since(self, earlier: UnixMs) -> DurationMs {
        DurationMs(self.0.saturating_sub(earlier.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_never_goes_negative() {
        assert_eq!(UnixMs(5_000).since(UnixMs(3_000)), DurationMs(2_000));
        assert_eq!(UnixMs(3_000).since(UnixMs(3_000)), DurationMs(0));
        assert_eq!(UnixMs(3_000).since(UnixMs(5_000)), DurationMs(0));
    }

    #[test]
    fn they_order_like_their_numbers() {
        assert!(UnixMs(1) < UnixMs(2));
        assert!(DurationMs(1_999) < DurationMs(2_000));
        assert_eq!([UnixMs(3), UnixMs(1)].iter().min(), Some(&UnixMs(1)));
    }

    #[test]
    fn on_the_wire_they_are_plain_numbers() {
        assert_eq!(serde_json::to_string(&UnixMs(42)).unwrap(), "42");
        assert_eq!(
            serde_json::to_string(&DurationMs(1_800_000)).unwrap(),
            "1800000"
        );
        assert_eq!(
            serde_json::from_str::<DurationMs>("7").unwrap(),
            DurationMs(7)
        );
    }
}
