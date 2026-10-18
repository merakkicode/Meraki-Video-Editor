use crate::VideoFrame;
use meraki_core::{AssetId, Time};
use std::{collections::VecDeque, sync::Arc};

/// Bounded LRU. Arc clones share the pixel allocation; byte accounting covers cache ownership.
pub struct FrameCache {
    frames: VecDeque<Arc<VideoFrame>>,
    capacity: usize,
    max_bytes: usize,
    bytes: usize,
}
impl FrameCache {
    pub fn new(capacity: usize, max_bytes: usize) -> Self {
        Self {
            frames: VecDeque::new(),
            capacity,
            max_bytes,
            bytes: 0,
        }
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn len(&self) -> usize {
        self.frames.len()
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    pub fn future_frames(&self, asset: AssetId, at: Time) -> usize {
        self.frames
            .iter()
            .filter(|f| f.asset == asset && f.pts > at)
            .count()
    }
    /// Streaming must evict consumed frames before upcoming frames. Pure LRU
    /// promotes the current frame and otherwise evicts the nearest future frame.
    pub fn discard_before(&mut self, asset: AssetId, at: Time) {
        self.frames.retain(|f| {
            f.asset != asset || !f.pts.checked_add(f.duration).is_ok_and(|end| end <= at)
        });
        self.bytes = self.frames.iter().map(|f| f.pixels.len()).sum();
    }
    pub fn insert(&mut self, frame: Arc<VideoFrame>) {
        if self.capacity == 0 || frame.pixels.len() > self.max_bytes {
            return;
        }
        if let Some(i) = self
            .frames
            .iter()
            .position(|f| f.asset == frame.asset && f.pts == frame.pts)
        {
            self.bytes -= self.frames.remove(i).unwrap().pixels.len();
        }
        while self.frames.len() >= self.capacity || self.bytes + frame.pixels.len() > self.max_bytes
        {
            self.bytes -= self.frames.pop_front().unwrap().pixels.len();
        }
        self.bytes += frame.pixels.len();
        self.frames.push_back(frame);
    }
    pub fn get(&mut self, asset: AssetId, at: Time) -> Option<Arc<VideoFrame>> {
        let index = self
            .frames
            .iter()
            .rposition(|f| f.asset == asset && f.covers(at))?;
        let f = self.frames.remove(index)?;
        self.frames.push_back(f.clone());
        Some(f)
    }
    pub fn clear(&mut self) {
        self.frames.clear();
        self.bytes = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;
    fn frame(n: i64) -> Arc<VideoFrame> {
        Arc::new(VideoFrame {
            asset: AssetId(1),
            format: PixelFormat::Rgba8,
            width: 2,
            height: 2,
            stride: 8,
            pts: Time::new(n, 30).unwrap(),
            duration: Time::new(1, 30).unwrap(),
            pixels: Arc::new(vec![0; 16]),
        })
    }
    #[test]
    fn bounded_lru_preserves_recent_hits() {
        let mut c = FrameCache::new(3, 32);
        c.insert(frame(0));
        c.insert(frame(1));
        assert_eq!(c.future_frames(AssetId(1), Time::ZERO), 1);
        assert!(c.get(AssetId(1), Time::ZERO).is_some());
        c.insert(frame(2));
        assert!(c.get(AssetId(1), Time::new(1, 30).unwrap()).is_none());
        assert_eq!(c.bytes(), 32);
        assert_eq!(c.len(), 2);
    }
    #[test]
    fn playback_eviction_keeps_upcoming_frames() {
        let mut c = FrameCache::new(3, 48);
        for n in 0..3 {
            c.insert(frame(n));
        }
        c.get(AssetId(1), Time::ZERO).unwrap();
        c.get(AssetId(1), Time::new(1, 30).unwrap()).unwrap();
        c.discard_before(AssetId(1), Time::new(1, 30).unwrap());
        c.insert(frame(3));
        assert!(c.get(AssetId(1), Time::new(2, 30).unwrap()).is_some());
        assert_eq!(c.bytes(), 48);
    }
    #[test]
    fn zero_budget_and_half_open_ranges() {
        let mut c = FrameCache::new(1, 15);
        c.insert(frame(0));
        assert!(c.is_empty());
        let mut c = FrameCache::new(1, 16);
        c.insert(frame(0));
        assert!(c.get(AssetId(1), Time::new(1, 30).unwrap()).is_none());
        c.insert(frame(0));
        assert_eq!(c.bytes(), 16);
        c.clear();
        assert_eq!(c.bytes(), 0);
    }
}
