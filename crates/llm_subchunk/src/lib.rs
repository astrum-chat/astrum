use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A single sub-chunk to send, with an optional delay to wait *before* sending.
pub struct SubChunk {
    pub text: String,
    pub delay: Option<Duration>,
}

/// Iterator over sub-chunks returned by [`SubChunker::process`].
///
/// Owns its sub-chunks independently. On drop, any unconsumed sub-chunks
/// are appended to the parent [`SubChunker`]'s unsent buffer so that
/// [`SubChunker::flush`] can recover them after an abort.
pub struct SubChunkIter<'a> {
    chunks: VecDeque<SubChunk>,
    unsent: &'a mut String,
}

impl Iterator for SubChunkIter<'_> {
    type Item = SubChunk;

    fn next(&mut self) -> Option<SubChunk> {
        self.chunks.pop_front()
    }
}

impl Drop for SubChunkIter<'_> {
    fn drop(&mut self) {
        for sub in self.chunks.drain(..) {
            self.unsent.push_str(&sub.text);
        }
    }
}

/// Splits incoming text into small sub-chunks and computes inter-sub-chunk
/// delays using a bias-corrected EMA of observed inter-chunk intervals.
pub struct SubChunker {
    subchunk_len: usize,
    smoothing_factor: f64,
    last_chunk_time: Option<Instant>,
    avg_interval: Option<Duration>,
    ema_raw: f64,
    interval_count: u32,
    is_first_chunk: bool,
    unsent: String,
}

impl Default for SubChunker {
    fn default() -> Self {
        Self::new(3, 0.45)
    }
}

impl SubChunker {
    /// Create a new `SubChunker`.
    ///
    /// * `subchunk_len` — target number of characters per sub-chunk.
    /// * `smoothing_factor` — EMA smoothing factor (0.0–1.0). 0.3 adapts within ~5 chunks.
    pub fn new(subchunk_len: usize, smoothing_factor: f64) -> Self {
        Self {
            subchunk_len,
            smoothing_factor,
            last_chunk_time: None,
            avg_interval: None,
            ema_raw: 0.0,
            interval_count: 0,
            is_first_chunk: true,
            unsent: String::new(),
        }
    }

    /// Processes a chunk of text. Records timing and splits the text into
    /// sub-chunks returned as an iterator.
    pub fn process(&mut self, text: &str) -> SubChunkIter<'_> {
        let now = Instant::now();

        // Skip the first interval (chunk 0 → chunk 1) because it includes
        // time-to-first-token latency which would inflate the average.
        if self.is_first_chunk {
            self.is_first_chunk = false;
        } else if let Some(last_time) = self.last_chunk_time {
            let elapsed = now.duration_since(last_time);
            self.interval_count += 1;

            // Bias-corrected EMA (same technique as the Adam optimizer).
            // Raw EMA is biased toward zero for the first few samples;
            // dividing by (1 - (1-α)^t) compensates exactly, giving an
            // accurate average from the very first interval.
            self.ema_raw = self.smoothing_factor * elapsed.as_secs_f64()
                + (1.0 - self.smoothing_factor) * self.ema_raw;
            let correction = 1.0 - (1.0 - self.smoothing_factor).powi(self.interval_count as i32);
            let corrected = self.ema_raw / correction;
            self.avg_interval = Some(Duration::from_secs_f64(corrected));
        }
        self.last_chunk_time = Some(now);

        let parts = split_into_subchunks(text, self.subchunk_len);
        let num_parts = parts.len();

        let delay = if num_parts > 1 {
            self.avg_interval.map(|avg| avg / (num_parts as u32 - 1))
        } else {
            None
        };

        let chunks: VecDeque<SubChunk> = parts
            .into_iter()
            .enumerate()
            .map(|(i, part)| SubChunk {
                text: part.to_string(),
                delay: if i > 0 { delay } else { None },
            })
            .collect();

        SubChunkIter {
            chunks,
            unsent: &mut self.unsent,
        }
    }

    /// Flush and return any text that hasn't been sent yet.
    /// Returns `None` if everything has been sent.
    pub fn flush(&mut self) -> Option<String> {
        if self.unsent.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.unsent))
    }
}

/// Splits `text` into sub-chunks of approximately `size` characters,
/// respecting Unicode character boundaries.
pub fn split_into_subchunks(text: &str, size: usize) -> Vec<&str> {
    if size == 0 || text.is_empty() {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    for (i, (byte_idx, _)) in text.char_indices().enumerate() {
        if i > 0 && i % size == 0 {
            chunks.push(&text[start..byte_idx]);
            start = byte_idx;
        }
    }

    if start < text.len() {
        chunks.push(&text[start..]);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_empty() {
        let result = split_into_subchunks("", 3);
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn test_split_shorter_than_size() {
        let result = split_into_subchunks("ab", 3);
        assert_eq!(result, vec!["ab"]);
    }

    #[test]
    fn test_split_exact_size() {
        let result = split_into_subchunks("abc", 3);
        assert_eq!(result, vec!["abc"]);
    }

    #[test]
    fn test_split_multiple_chunks() {
        let result = split_into_subchunks("abcdef", 3);
        assert_eq!(result, vec!["abc", "def"]);
    }

    #[test]
    fn test_split_with_remainder() {
        let result = split_into_subchunks("abcdefg", 3);
        assert_eq!(result, vec!["abc", "def", "g"]);
    }

    #[test]
    fn test_split_unicode() {
        let result = split_into_subchunks("héllo", 2);
        assert_eq!(result, vec!["hé", "ll", "o"]);
    }

    #[test]
    fn test_split_zero_size() {
        let result = split_into_subchunks("abc", 0);
        assert_eq!(result, vec!["abc"]);
    }

    #[test]
    fn test_subchunker_yields_correct_subchunks() {
        let mut sc = SubChunker::new(3, 0.3);
        let mut iter = sc.process("abcdef");

        let first = iter.next().unwrap();
        assert_eq!(first.text, "abc");
        assert!(first.delay.is_none());

        let second = iter.next().unwrap();
        assert_eq!(second.text, "def");
        // No timing info yet, so delay is None.
        assert!(second.delay.is_none());

        assert!(iter.next().is_none());
    }

    #[test]
    fn test_subchunker_flush_nothing_consumed() {
        let mut sc = SubChunker::new(3, 0.3);
        drop(sc.process("abcdef"));
        assert_eq!(sc.flush(), Some("abcdef".to_string()));
        assert_eq!(sc.flush(), None);
    }

    #[test]
    fn test_subchunker_flush_partial() {
        let mut sc = SubChunker::new(3, 0.3);
        let mut iter = sc.process("abcdef");
        iter.next(); // consumes "abc"
        drop(iter);
        assert_eq!(sc.flush(), Some("def".to_string()));
    }

    #[test]
    fn test_subchunker_flush_all_consumed() {
        let mut sc = SubChunker::new(3, 0.3);
        let subs: Vec<_> = sc.process("abcdef").collect();
        assert_eq!(subs.len(), 2);
        assert_eq!(sc.flush(), None);
    }

    #[test]
    fn test_subchunker_flush_across_chunks() {
        let mut sc = SubChunker::new(3, 0.3);
        // First chunk fully consumed.
        let _: Vec<_> = sc.process("abc").collect();
        // Second chunk partially consumed (iterator dropped after one sub-chunk).
        let mut iter = sc.process("defghi");
        iter.next(); // consumes "def"
        drop(iter);
        // flush should return the unconsumed "ghi".
        assert_eq!(sc.flush(), Some("ghi".to_string()));
    }
}
