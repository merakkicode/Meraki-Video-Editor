use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
/// Safe SPSC: private endpoints enforce one writer and one reader. Slots hold copyable bits only.
struct Ring {
    slots: Box<[AtomicU64]>,
    read: AtomicUsize,
    write: AtomicUsize,
}
pub struct Producer(Arc<Ring>);
pub struct Consumer(Arc<Ring>);
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let r = Arc::new(Ring {
        slots: (0..capacity + 1).map(|_| AtomicU64::new(0)).collect(),
        read: AtomicUsize::new(0),
        write: AtomicUsize::new(0),
    });
    (Producer(r.clone()), Consumer(r))
}
impl Producer {
    pub fn push(&mut self, frame: [f32; 2]) -> bool {
        let r = &self.0;
        let w = r.write.load(Ordering::Relaxed);
        let next = (w + 1) % r.slots.len();
        if next == r.read.load(Ordering::Acquire) {
            return false;
        }
        r.slots[w].store(
            u64::from(frame[0].to_bits()) | (u64::from(frame[1].to_bits()) << 32),
            Ordering::Relaxed,
        );
        r.write.store(next, Ordering::Release);
        true
    }
    pub fn len(&self) -> usize {
        let r = &self.0;
        (r.write.load(Ordering::Relaxed) + r.slots.len() - r.read.load(Ordering::Acquire))
            % r.slots.len()
    }
}
impl Consumer {
    pub fn pop(&mut self) -> Option<[f32; 2]> {
        let r = &self.0;
        let i = r.read.load(Ordering::Relaxed);
        if i == r.write.load(Ordering::Acquire) {
            return None;
        }
        let bits = r.slots[i].load(Ordering::Relaxed);
        r.read.store((i + 1) % r.slots.len(), Ordering::Release);
        Some([
            f32::from_bits(bits as u32),
            f32::from_bits((bits >> 32) as u32),
        ])
    }
    pub fn clear(&mut self) {
        self.0
            .read
            .store(self.0.write.load(Ordering::Acquire), Ordering::Release);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_wrap_and_clear() {
        let (mut p, mut c) = ring(2);
        assert!(p.push([1., 2.]));
        assert!(p.push([3., 4.]));
        assert!(!p.push([5., 6.]));
        assert_eq!(c.pop(), Some([1., 2.]));
        assert!(p.push([5., 6.]));
        c.clear();
        assert_eq!(c.pop(), None);
        assert_eq!(p.len(), 0);
    }
    #[test]
    fn concurrent_order() {
        let (mut p, mut c) = ring(17);
        let t = std::thread::spawn(move || {
            for i in 0..10000 {
                while !p.push([i as f32, -(i as f32)]) {
                    std::thread::yield_now();
                }
            }
        });
        for i in 0..10000 {
            let f = loop {
                if let Some(f) = c.pop() {
                    break f;
                }
                std::thread::yield_now();
            };
            assert_eq!(f, [i as f32, -(i as f32)]);
        }
        t.join().unwrap();
    }
}
