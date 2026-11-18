//! The play queue (web `lib/audio/queue.ts`).

use super::url::is_surah_number;

/// `QueueSegment`: a contiguous run of ayahs in one surah.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueSegment {
    pub surah: u16,
    pub from: u16,
    pub to: u16,
}

/// `QueueItem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueueItem {
    pub surah: u16,
    pub ayah: u16,
}

/// `PlayQueue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayQueue {
    pub segments: Vec<QueueSegment>,
    /// Every ayah in play order.
    pub items: Vec<QueueItem>,
    pub index: usize,
}

/// `MAX_QUEUE_ITEMS`.
pub const MAX_QUEUE_ITEMS: usize = 1000;

/// `isValidSegment`.
pub fn is_valid_segment(s: QueueSegment) -> bool {
    is_surah_number(s.surah) && s.from >= 1 && s.to >= s.from
}

/// `buildQueue`: `None` for no segments, an invalid one, or more than
/// [`MAX_QUEUE_ITEMS`].
pub fn build_queue(segments: &[QueueSegment]) -> Option<PlayQueue> {
    if segments.is_empty() {
        return None;
    }
    let mut items = Vec::new();
    for &segment in segments {
        if !is_valid_segment(segment) {
            return None;
        }
        for ayah in segment.from..=segment.to {
            items.push(QueueItem {
                surah: segment.surah,
                ayah,
            });
            if items.len() > MAX_QUEUE_ITEMS {
                return None;
            }
        }
    }
    Some(PlayQueue {
        segments: segments.to_vec(),
        items,
        index: 0,
    })
}

impl PlayQueue {
    /// `queueAt`.
    pub fn at(&self, index: usize) -> Option<PlayQueue> {
        (index < self.items.len()).then(|| PlayQueue {
            index,
            ..self.clone()
        })
    }

    /// `stepQueue`: one ayah forward (`forward`) or back. `None` at either end.
    pub fn step(&self, forward: bool) -> Option<PlayQueue> {
        if forward {
            return self.at(self.index + 1);
        }
        self.index.checked_sub(1).and_then(|i| self.at(i))
    }

    /// `currentItem`.
    pub fn current(&self) -> Option<QueueItem> {
        self.items.get(self.index).copied()
    }

    /// `surahBoundary`: index of the first ayah of the next (`forward`) or
    /// previous surah in the queue.
    pub fn surah_boundary(&self, forward: bool) -> Option<usize> {
        let current = self.current()?;
        if forward {
            return (self.index + 1..self.items.len())
                .find(|&j| self.items[j].surah != current.surah);
        }
        let mut j = self.index;
        while j > 0 && self.items[j - 1].surah == current.surah {
            j -= 1;
        }
        // `j` is the first item of the current surah run.
        if j == 0 {
            return None;
        }
        let previous = self.items[j - 1].surah;
        let mut k = j - 1;
        while k > 0 && self.items[k - 1].surah == previous {
            k -= 1;
        }
        Some(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(surah: u16, from: u16, to: u16) -> QueueSegment {
        QueueSegment { surah, from, to }
    }

    #[test]
    fn builds_and_steps() {
        let q = build_queue(&[seg(1, 6, 7), seg(2, 1, 2)]).unwrap();
        assert_eq!(q.items.len(), 4);
        assert_eq!(q.step(false), None);
        let q = q.step(true).unwrap();
        assert_eq!(q.current(), Some(QueueItem { surah: 1, ayah: 7 }));
        assert_eq!(q.at(4), None);
    }

    #[test]
    fn rejects_bad_queues() {
        assert!(build_queue(&[]).is_none());
        assert!(build_queue(&[seg(0, 1, 1)]).is_none());
        assert!(build_queue(&[seg(1, 3, 2)]).is_none());
        assert!(build_queue(&[seg(2, 1, 1001)]).is_none());
        assert!(build_queue(&[seg(2, 1, 1000)]).is_some());
    }

    #[test]
    fn surah_boundaries() {
        let q = build_queue(&[seg(1, 1, 3), seg(2, 1, 2), seg(3, 1, 2)]).unwrap();
        assert_eq!(q.surah_boundary(true), Some(3));
        assert_eq!(q.surah_boundary(false), None);
        let mid = q.at(4).unwrap();
        assert_eq!(mid.surah_boundary(false), Some(0));
        assert_eq!(mid.surah_boundary(true), Some(5));
        let last = q.at(6).unwrap();
        assert_eq!(last.surah_boundary(true), None);
        assert_eq!(last.surah_boundary(false), Some(3));
    }
}
