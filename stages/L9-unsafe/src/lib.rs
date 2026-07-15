//! L9 作业：RingBuffer 安全封装——unsafe 的信任边界
//!
//! 高频采样需要一个定长环形缓冲：满了就覆盖最老的，创建后**零堆分配**。
//! 内部用 `MaybeUninit`（不初始化的存储）+ 裸操作，外部是安全 API。
//!
//! 不变量（unsafe 的每一条都靠它成立）：
//!   - `buf` 长度恒为 `capacity`（创建后不变）；
//!   - 逻辑上第 0..len 个元素**已初始化**，其余是 uninit；
//!   - 最老元素的物理下标 = `(head + cap - len) % cap`；`head` 是下次写入位置。
//!
//! `with_capacity` / `len` / `Drop` 是讲师带敲的基础骨架（已给出）；
//! 你的任务是补全 `push` 和 `get` 的 unsafe 部分，并让 `cargo +nightly miri test` 干净。

use std::mem::MaybeUninit;

pub struct Ring<T> {
    buf: Box<[MaybeUninit<T>]>,
    head: usize, // 下次写入的物理下标
    len: usize,  // 有效元素个数（<= capacity）
}

impl<T> Ring<T> {
    /// 一次性分配 `cap` 个槽位，之后 push 不再分配。
    pub fn with_capacity(cap: usize) -> Self {
        assert!(cap > 0, "容量必须为正");
        let buf = (0..cap)
            .map(|_| MaybeUninit::uninit())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ring {
            buf,
            head: 0,
            len: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 物理下标：逻辑第 `i` 老的元素（`i < len`）落在哪个槽。
    fn phys(&self, i: usize) -> usize {
        let cap = self.buf.len();
        let oldest = (self.head + cap - self.len) % cap;
        (oldest + i) % cap
    }

    /// 追加一个元素。满了就**覆盖最老的**（先把它 drop 掉，避免泄漏）。
    pub fn push(&mut self, value: T) {
        todo!("L9：写入 head 槽；满时先 drop 被覆盖的最老元素；维护 head/len 不变量")
    }

    /// 读逻辑第 `i` 老的元素（0 = 最老）。越界返回 `None`。
    pub fn get(&self, i: usize) -> Option<&T> {
        todo!("L9：i>=len 返回 None；否则借用对应槽的已初始化值")
    }
}

impl<T> Drop for Ring<T> {
    fn drop(&mut self) {
        // 只 drop 已初始化的 len 个元素，uninit 槽不能碰。
        for i in 0..self.len {
            let idx = self.phys(i);
            // SAFETY: 0..len 的逻辑元素都已初始化，每个恰好 drop 一次。
            unsafe { self.buf[idx].assume_init_drop() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn push_and_get_in_order() {
        let mut r = Ring::with_capacity(3);
        r.push(10);
        r.push(20);
        assert_eq!(r.len(), 2);
        assert_eq!(r.get(0), Some(&10)); // 最老
        assert_eq!(r.get(1), Some(&20));
        assert_eq!(r.get(2), None); // 越界
    }

    #[test]
    fn overwrites_oldest_when_full() {
        let mut r = Ring::with_capacity(3);
        for v in [1, 2, 3, 4, 5] {
            r.push(v);
        }
        // 容量 3，最后三个是 3,4,5
        assert_eq!(r.len(), 3);
        assert_eq!(r.get(0), Some(&3));
        assert_eq!(r.get(1), Some(&4));
        assert_eq!(r.get(2), Some(&5));
    }

    /// 关键：被覆盖的元素、以及析构时的剩余元素，都必须 drop 恰好一次。
    /// 用一个 Drop 计数器 + miri 一起把"泄漏 / double-free"挡住。
    #[test]
    fn every_element_dropped_exactly_once() {
        struct Tracked(Arc<AtomicUsize>);
        impl Drop for Tracked {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        {
            let mut r = Ring::with_capacity(2);
            for _ in 0..5 {
                r.push(Tracked(Arc::clone(&drops)));
            }
            // push 了 5 个，其中 3 个被覆盖 → 已 drop 3 个
            assert_eq!(drops.load(Ordering::SeqCst), 3);
        } // Ring drop：剩下 2 个
        assert_eq!(drops.load(Ordering::SeqCst), 5);
    }
}
