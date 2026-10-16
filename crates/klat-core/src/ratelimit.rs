//! How often the event server warns about rejected requests: a burst, then a count. Something hammering the
//! port must not fill the log.

/// Warnings logged per window; the rest of the window is only counted.
pub const REJECT_LOG_BURST: u32 = 5;
pub const REJECT_LOG_WINDOW_MS: u64 = 60_000;

/// Rate limit for the rejection warnings (`REJECT_LOG_BURST` per `REJECT_LOG_WINDOW_MS`).
#[derive(Debug, Default)]
pub struct RejectLog {
    window_start: u64,
    logged: u32,
    suppressed: u32,
}

impl RejectLog {
    pub const fn new() -> Self {
        Self {
            window_start: 0,
            logged: 0,
            suppressed: 0,
        }
    }

    /// The log lines for a rejection at `now` (unix ms): none once the burst is used up.
    pub fn on_reject(&mut self, status: u16, now: u64) -> Vec<String> {
        let mut lines = Vec::new();
        if now.saturating_sub(self.window_start) >= REJECT_LOG_WINDOW_MS {
            if self.suppressed > 0 {
                lines.push(format!(
                    "{} more rejected requests to the event endpoint were not logged",
                    self.suppressed
                ));
            }
            *self = Self {
                window_start: now,
                ..Self::new()
            };
        }
        if self.logged < REJECT_LOG_BURST {
            self.logged += 1;
            let mut line = format!("rejected a request to the event endpoint: HTTP {status}");
            if self.logged == REJECT_LOG_BURST {
                line.push_str(" (more this minute are counted, not logged)");
            }
            lines.push(line);
        } else {
            self.suppressed += 1;
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejections_are_logged_in_bursts_and_the_rest_counted() {
        let mut log = RejectLog::default();
        let t0 = 1_000_000;
        let lines: Vec<_> = (0..20).flat_map(|i| log.on_reject(401, t0 + i)).collect();
        assert_eq!(lines.len(), REJECT_LOG_BURST as usize);
        assert!(lines.last().unwrap().contains("not logged"), "{lines:?}");
        // Next window: one line for what was dropped, then logging resumes.
        let next = log.on_reject(404, t0 + REJECT_LOG_WINDOW_MS);
        assert_eq!(next.len(), 2, "{next:?}");
        assert!(next[0].contains("15 more"), "{next:?}");
        assert!(next[1].contains("HTTP 404"), "{next:?}");
    }
}
