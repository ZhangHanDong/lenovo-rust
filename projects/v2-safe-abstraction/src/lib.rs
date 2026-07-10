//! 第 6 课核心逻辑：用**少量 `unsafe` 构建一个对外完全安全的抽象**。
//!
//! 本 crate 不是为了"实现一个容器"，而是把第 6 课讲的"安全抽象（safe
//! abstraction）"方法论落到可编译、可测试、可被 miri 检验的代码里：
//!
//! - **把 `unsafe` 关进最小封装**：整个 [`RingBuffer`] 只有 4 处 `unsafe`，
//!   全部锁在私有方法内部；对外暴露的 `push_back` / `pop_front` / `front`
//!   / `len` 等 API **没有一个是 `unsafe` 的**，调用方无论怎么用都不会触发
//!   未定义行为（UB）。
//! - **未初始化内存的正确管理**：底层用 `Box<[MaybeUninit<T>]>` 持有一段
//!   **可能未初始化**的槽位，靠 `head`/`len` 两个下标精确记录"哪些槽位是
//!   活的（已初始化）"，从而避免"读未初始化内存"和"析构未初始化内存"这两类 UB。
//! - **`// SAFETY:` 不变量注释**：每个 `unsafe` 块都写明"我依赖什么前提、
//!   维持了什么不变量"。这正是 `unsafe` 代码审查的抓手——审查者只需逐条核对
//!   SAFETY 注释里的前提是否真的成立。
//! - **`Drop` 不泄漏、不重复析构**：自定义 `Drop` 只析构 `[head, head+len)`
//!   区间内的已初始化元素，槽位被 `pop` 取走后即视为未初始化，绝不二次析构。
//!
//! 赏析锚点：`bytes`（`Bytes`/`BytesMut` 用引用计数 + 裸指针实现零拷贝切分，
//! 对外是安全的字节视图）与 `smallvec`（栈上内联存储 + 溢出转堆，内部大量
//! `MaybeUninit`/裸指针，对外是 `Vec` 般的安全 API）——都是"少量 unsafe，
//! 对外零 unsafe"的工业级范本。本 crate 是它们的微缩教学版。
//!
//! # 不变量（invariant）
//!
//! `RingBuffer<T>` 在任意时刻满足：
//!
//! 1. `capacity == buf.len()`（容量等于底层槽位数，构造后不变）；
//! 2. `len <= capacity`；
//! 3. 当 `capacity > 0` 时 `head < capacity`；
//! 4. **恰好** `buf` 中逻辑区间 `[head, head+len)`（按容量回绕取模）对应的
//!    槽位是**已初始化**的，其余槽位均**未初始化**。
//!
//! 所有 `unsafe` 的正确性都建立在第 4 条之上。
//!
//! # 用法
//!
//! ```
//! use v2_safe_abstraction::RingBuffer;
//!
//! let mut rb = RingBuffer::with_capacity(2);
//! assert!(rb.push_back(1).is_ok());
//! assert!(rb.push_back(2).is_ok());
//! // 满了：把所有权原样还给调用方，而不是 panic
//! assert_eq!(rb.push_back(3), Err(3));
//!
//! assert_eq!(rb.pop_front(), Some(1));
//! assert!(rb.push_back(3).is_ok()); // 腾出空位后绕回写入
//! assert_eq!(rb.pop_front(), Some(2));
//! assert_eq!(rb.pop_front(), Some(3));
//! assert_eq!(rb.pop_front(), None);
//! ```

#![forbid(unsafe_op_in_unsafe_fn)]

use std::mem::MaybeUninit;

/// 固定容量的环形缓冲区（ring buffer / circular buffer）。
///
/// 内部用一段**可能未初始化**的连续槽位 `Box<[MaybeUninit<T>]>` 配合
/// `head`/`len` 两个下标实现先进先出（FIFO）队列。写满时 [`push_back`] 不会
/// 覆盖旧数据，而是把待入队的值原样返回给调用方（`Err(value)`）。
///
/// 对外 API 全部是安全的；内部仅在"取出/借用/析构已初始化槽位"处使用 `unsafe`。
///
/// [`push_back`]: RingBuffer::push_back
pub struct RingBuffer<T> {
    /// 底层槽位。长度即容量，构造后不再变化。
    buf: Box<[MaybeUninit<T>]>,
    /// 队首（下一个被 `pop` 的元素）在 `buf` 中的物理下标。`capacity > 0` 时恒 `< capacity`。
    head: usize,
    /// 当前已初始化（在队中的）元素个数。`len <= capacity`。
    len: usize,
}

impl<T> RingBuffer<T> {
    /// 创建一个容量为 `capacity` 的空环形缓冲区。
    ///
    /// `capacity == 0` 是合法的：这样的缓冲区永远是满的（无法 `push`）、
    /// 永远是空的（无法 `pop`），所有访问路径都被边界检查短路，不触发任何 `unsafe`。
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        // (0..capacity) 个未初始化槽位。MaybeUninit::uninit() 不读写内存，
        // 也不要求 T: Default —— 这正是它相对 `vec![T::default(); n]` 的价值。
        let buf = (0..capacity)
            .map(|_| MaybeUninit::uninit())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            buf,
            head: 0,
            len: 0,
        }
    }

    /// 容量（底层槽位数）。
    #[must_use]
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// 当前在队元素个数。
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// 是否为空。
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 是否已满。
    #[must_use]
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == self.capacity()
    }

    /// 把逻辑偏移 `logical` 映射为 `buf` 中的物理下标。
    ///
    /// **前置条件（契约）**：`logical < capacity`。调用方一览：`pop_front`/`front`
    /// 传 `0`、`push_back` 传 `len`（未满时 `len < capacity`）、`Drop` 遍历
    /// `[0, len)`——全部满足该前置条件。注意 `pop_front` 前移 `head` 时**不**复用
    /// `physical_index(1)`：`capacity == 1` 时 `1 == capacity` 会违反此契约。
    ///
    /// 因为 `head < capacity` 且 `logical < capacity`，所以 `head + logical < 2*capacity`，
    /// 一次减法即可完成回绕，无需取模（除法）。返回值恒 `< capacity`。
    #[inline]
    fn physical_index(&self, logical: usize) -> usize {
        debug_assert!(logical < self.capacity());
        debug_assert!(self.head < self.capacity());
        let idx = self.head + logical;
        let cap = self.capacity();
        if idx >= cap {
            idx - cap
        } else {
            idx
        }
    }

    /// 入队到队尾。
    ///
    /// - 成功：返回 `Ok(())`。
    /// - 已满：**不覆盖**任何旧数据，把 `value` 原样还给调用方 `Err(value)`。
    pub fn push_back(&mut self, value: T) -> Result<(), T> {
        if self.is_full() {
            return Err(value);
        }
        // 目标槽位是"最后一个在队元素的下一个"，即第 `len` 个逻辑位置。
        let idx = self.physical_index(self.len);
        // SAFETY: `physical_index` 保证 `idx < capacity`，故 `get_unchecked_mut`
        // 不越界（避免"越界访问"UB）。该槽位位于 `[head, head+len)` 之外，按不变量(4)
        // 是**未初始化**的，因此用 `MaybeUninit::write`（安全方法）写入不会丢弃任何
        // 旧值的析构、也不读取未初始化内存。写入后该槽位成为已初始化。
        unsafe { self.buf.get_unchecked_mut(idx) }.write(value);
        // 维持不变量(4)：区间扩张为 [head, head+len+1)。
        self.len += 1;
        Ok(())
    }

    /// 从队首出队。空时返回 `None`。
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        // 队首逻辑下标 0 对应的物理槽位，即 head。
        let idx = self.physical_index(0);
        // SAFETY: `idx == head < capacity`，故 `get_unchecked` 不越界。该槽位是队首，
        // 按不变量(4)处于 `[head, head+len)` 内，**已初始化**，因此 `assume_init_read`
        // 读出一个有效值不构成"读未初始化内存"UB。`assume_init_read` 仅按位拷贝出值，
        // 把所有权交给调用方；随后我们立刻收缩区间（head 前移、len 减一），令该槽位
        // 重新被视为未初始化——这保证它**不会被二次读取或在 Drop 中二次析构**。
        let value = unsafe { self.buf.get_unchecked(idx).assume_init_read() };
        // 维持不变量(3)(4)：队首前移一格（带回绕），区间收缩为 [head+1, head+len)。
        // 注意：不能写成 `self.head = self.physical_index(1)`——`physical_index`
        // 的契约要求 `logical < capacity`，而 `capacity == 1` 时 `1 == capacity`
        // 会违反契约（debug 构建下触发 debug_assert panic）。这里直接前移并回绕。
        self.head += 1;
        if self.head == self.capacity() {
            self.head = 0;
        }
        self.len -= 1;
        Some(value)
    }

    /// 借用队首元素（不出队）。空时返回 `None`。
    #[must_use]
    pub fn front(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        let idx = self.physical_index(0);
        // SAFETY: `idx == head < capacity`，`get_unchecked` 不越界；队首槽位按不变量(4)
        // 已初始化，故 `assume_init_ref` 返回的 `&T` 指向有效值。返回引用的生命周期被
        // 绑定到 `&self`，借用检查器保证它不会比缓冲区活得更久（无悬垂）。
        Some(unsafe { self.buf.get_unchecked(idx).assume_init_ref() })
    }

    /// 清空缓冲区：析构所有在队元素，长度归零。
    pub fn clear(&mut self) {
        // 复用 pop 的语义：逐个 pop 会触发每个元素的析构（值离开作用域）。
        while self.pop_front().is_some() {}
    }
}

impl<T> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        // 只析构**已初始化**的元素，即逻辑区间 [0, len)。其余槽位从未被写入，
        // 析构它们会造成"析构未初始化内存"UB。
        for logical in 0..self.len {
            let idx = self.physical_index(logical);
            // SAFETY: `physical_index` 保证 `idx < capacity`，不越界；`logical < len`
            // 说明该槽位落在 `[head, head+len)` 内，按不变量(4)已初始化，故对它调用
            // `assume_init_drop`（就地析构）是合法的。每个 logical 只遍历一次，不会
            // 对同一槽位二次析构。`Box` 自身的内存随后由编译器自动释放。
            unsafe { self.buf.get_unchecked_mut(idx).assume_init_drop() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;
    use std::cell::Cell;
    use std::rc::Rc;

    /// 析构计数器：每次 `drop` 把共享计数 +1，用于验证"不泄漏、不重复析构"。
    /// 派生 `Debug` 以便 `push_back(..).unwrap()` 在出错时能打印（`Err` 类型需 `Debug`）。
    #[derive(Debug)]
    struct Bomb {
        counter: Rc<Cell<usize>>,
    }

    impl Drop for Bomb {
        fn drop(&mut self) {
            self.counter.set(self.counter.get() + 1);
        }
    }

    #[test]
    fn fill_until_full_then_reject() {
        let mut rb = RingBuffer::with_capacity(3);
        assert!(rb.is_empty());
        assert_eq!(rb.push_back(10), Ok(()));
        assert_eq!(rb.push_back(20), Ok(()));
        assert_eq!(rb.push_back(30), Ok(()));
        assert!(rb.is_full());
        assert_eq!(rb.len(), 3);
        // 满了：原样退回所有权，不覆盖旧数据。
        assert_eq!(rb.push_back(40), Err(40));
        // 旧数据完好，FIFO 顺序正确。
        assert_eq!(rb.front(), Some(&10));
        assert_eq!(rb.pop_front(), Some(10));
        assert_eq!(rb.pop_front(), Some(20));
        assert_eq!(rb.pop_front(), Some(30));
    }

    #[test]
    fn drain_empty_returns_none() {
        let mut rb: RingBuffer<i32> = RingBuffer::with_capacity(2);
        assert_eq!(rb.front(), None);
        assert_eq!(rb.pop_front(), None);
        rb.push_back(1).unwrap();
        assert_eq!(rb.pop_front(), Some(1));
        // 再次读空：依然安全返回 None，不读未初始化内存。
        assert_eq!(rb.pop_front(), None);
        assert_eq!(rb.pop_front(), None);
    }

    #[test]
    fn wrap_around_preserves_fifo_order() {
        let mut rb = RingBuffer::with_capacity(3);
        rb.push_back(1).unwrap();
        rb.push_back(2).unwrap();
        assert_eq!(rb.pop_front(), Some(1)); // head 前移到 1
        rb.push_back(3).unwrap();
        rb.push_back(4).unwrap(); // 物理下标回绕到 0
        assert!(rb.is_full());
        // 缓冲区现在逻辑上是 [2, 3, 4]，且物理上发生了回绕。
        assert_eq!(rb.pop_front(), Some(2));
        assert_eq!(rb.pop_front(), Some(3));
        assert_eq!(rb.pop_front(), Some(4));
        assert_eq!(rb.pop_front(), None);
    }

    #[test]
    fn drop_runs_exactly_once_per_live_element() {
        let counter = Rc::new(Cell::new(0usize));
        {
            let mut rb = RingBuffer::with_capacity(4);
            for _ in 0..3 {
                rb.push_back(Bomb {
                    counter: Rc::clone(&counter),
                })
                .unwrap();
            }
            // 取走一个：它在离开 `pop_front` 返回值作用域时被析构。
            drop(rb.pop_front());
            assert_eq!(counter.get(), 1, "pop 出来的元素应恰好析构一次");
            // 此处 rb 仍持有 2 个元素，离开作用域触发 Drop。
        }
        // 共创建 3 个 Bomb，全部恰好析构一次：1（pop）+ 2（Drop）。
        // 既不泄漏（== 3），也不二次析构（不会 > 3）。
        assert_eq!(counter.get(), 3, "应恰好析构 3 次：无泄漏、无重复析构");
    }

    #[test]
    fn drop_after_wrap_does_not_leak() {
        let counter = Rc::new(Cell::new(0usize));
        {
            let mut rb = RingBuffer::with_capacity(3);
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
            drop(rb.pop_front()); // 析构 1 个，head 前移，制造回绕态
                                  // 再塞两个，物理下标回绕；此时在队 3 个，发生了 wrap。
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
            assert_eq!(counter.get(), 1);
            // 离开作用域：Drop 必须正确处理"物理上回绕"的 3 个活元素。
        }
        // 共创建 4 个，全部析构一次（1 个 pop + 3 个 Drop）。
        assert_eq!(counter.get(), 4, "回绕状态下 Drop 仍应精确析构每个活元素");
    }

    #[test]
    fn clear_drops_all_and_resets() {
        let counter = Rc::new(Cell::new(0usize));
        let mut rb = RingBuffer::with_capacity(4);
        for _ in 0..3 {
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
        }
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(counter.get(), 3, "clear 应析构全部在队元素");
        // clear 之后可继续正常使用。
        rb.push_back(Bomb {
            counter: Rc::clone(&counter),
        })
        .unwrap();
        assert_eq!(rb.len(), 1);
        drop(rb);
        assert_eq!(counter.get(), 4);
    }

    /// 回归测试：`capacity == 1` 时 `pop_front` 前移 `head` 恰好落在回绕边界上。
    /// 旧实现用 `physical_index(1)` 前移，`1 == capacity` 违反其契约，
    /// debug 构建下 `debug_assert!(logical < capacity)` 会 panic。
    #[test]
    fn capacity_one_push_pop_roundtrip() {
        let mut rb = RingBuffer::with_capacity(1);
        assert!(rb.is_empty());
        rb.push_back(1).unwrap();
        assert!(rb.is_full());
        assert_eq!(rb.push_back(2), Err(2)); // 满：退回，不覆盖
        assert_eq!(rb.pop_front(), Some(1)); // 旧实现在这里 panic
        assert!(rb.is_empty());
        // 反复 push/pop 往返：head 每轮都回绕到 0，FIFO 语义始终正确。
        for i in 10..15 {
            rb.push_back(i).unwrap();
            assert_eq!(rb.front(), Some(&i));
            assert_eq!(rb.pop_front(), Some(i));
            assert_eq!(rb.pop_front(), None);
        }
    }

    /// `capacity == 1` 下的析构精确性：pop 出的元素与 Drop 各析构一次。
    #[test]
    fn capacity_one_drop_is_exact() {
        let counter = Rc::new(Cell::new(0usize));
        {
            let mut rb = RingBuffer::with_capacity(1);
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
            drop(rb.pop_front());
            assert_eq!(counter.get(), 1);
            rb.push_back(Bomb {
                counter: Rc::clone(&counter),
            })
            .unwrap();
            // 离开作用域：Drop 析构在队的 1 个元素。
        }
        assert_eq!(counter.get(), 2, "共 2 个元素，各恰好析构一次");
    }

    #[test]
    fn zero_capacity_is_always_full_and_empty() {
        let mut rb: RingBuffer<i32> = RingBuffer::with_capacity(0);
        assert!(rb.is_empty());
        assert!(rb.is_full());
        assert_eq!(rb.push_back(1), Err(1));
        assert_eq!(rb.pop_front(), None);
        assert_eq!(rb.front(), None);
    }
}
