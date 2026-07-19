//! L5+ 综合练习：事件历史链表——用链表打通所有权与类型系统
//!
//! 蓝本:too-many-lists 安全部分,换成 WinMon 事件历史语境。
//! 两个数据结构:
//!   - `History<T>`:Box 单链表(独占所有权,undo 栈);
//!   - `Shared<T>`:Rc 持久化链表(共享尾巴,历史分叉)。
//!
//! 全程零 unsafe、零 unwrap(测试除外)、零深拷贝 clone(Rc::clone 是计数,允许)。

use std::rc::Rc;

// ───────────────── 阶段二:Box 单链表 ─────────────────

/// 事件历史(undo 栈)。`head` 拥有第一个节点,每个节点拥有下一个——
/// L2 的"唯一属主"串成链。
pub struct History<T> {
    head: Option<Box<Node<T>>>,
    len: usize,
}

struct Node<T> {
    elem: T,
    next: Option<Box<Node<T>>>,
}

impl<T> History<T> {
    pub fn new() -> Self {
        History { head: None, len: 0 }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 新事件入栈:新节点接管整条旧链。
    /// ⚠️ 直接 `let old = self.head;` 是 E0507——借用着的字段不能搬空,
    ///    用 `Option::take` 先补 None 再拿走。
    pub fn push(&mut self, elem: T) {
        todo!("L5X push:用 self.head.take() 把旧链装进新节点的 next,再接回 head")
    }

    /// 回退:头节点交出元素,链子接给下一环。空表返回 None(不是错误,是状态)。
    pub fn pop(&mut self) -> Option<T> {
        todo!("L5X pop:take 出头节点,head 接 node.next,返回 node.elem")
    }

    /// 只看最新事件,不拿走(L3 的借用)。
    pub fn peek(&self) -> Option<&T> {
        todo!("L5X peek:as_deref 借到节点,map 出 elem 的引用")
    }

    pub fn peek_mut(&mut self) -> Option<&mut T> {
        self.head.as_deref_mut().map(|n| &mut n.elem)
    }

    /// 借用迭代器:遍历不消耗(L3 借用 + L5 Iterator trait)。
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            next: self.head.as_deref(),
        }
    }
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// 临时视图结构体:不能活得比 History 久('a 就是那份约束,见 L3+ §1.4)。
pub struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        todo!("L5X Iter:map 里让 self.next 走向 node.next.as_deref(),返回 elem 引用")
    }
}

// ── 三种迭代器补全:所有权的全谱系(L2/L3 三种传参在迭代器上的重演) ──

/// 消耗式迭代器:拿走整个 History(self),吐出拥有所有权的 T。
pub struct IntoIter<T>(History<T>);

impl<T> History<T> {
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            next: self.head.as_deref_mut(),
        }
    }
}

/// 实现标准 trait 而非固有方法——白赚 `for ev in history` 语法。
impl<T> IntoIterator for History<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        IntoIter(self)
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        todo!("L5X IntoIter:一行——复用你已写好的 pop")
    }
}

/// 可变迭代器——too-many-lists 称之为安全 Rust 的智力巅峰之一。
/// 难点:`&mut` 是独占的、不可复制——共享版 Iter 里 `self.next.map(...)` 能过
/// (因为 `&T` 可复制),这里同样写法是 E0500(闭包要独占 self.next,它却还被借着)。
/// 钥匙:`take`——把 `&mut Node` 整个从 self 里拿出来,用完把下一环放回去。
pub struct IterMut<'a, T> {
    next: Option<&'a mut Node<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        todo!("L5X IterMut:take 出 mut 引用,推进到 node.next.as_deref_mut,交出 mut elem")
    }
}

/// 隐藏关卡:不写这个 impl 也能编译——但默认析构是递归的,
/// 20 万节点的链会在 drop 时爆栈。把递归拍平成循环。
impl<T> Drop for History<T> {
    fn drop(&mut self) {
        todo!("L5X Drop:循环 take 每个节点,把递归析构拍平(注释掉试试爆栈)")
    }
}

// ───────────────── 阶段三:Rc 持久化链表 ─────────────────

/// 可分叉的共享历史:prepend 不动旧列表,新旧共享尾巴(像 git 分支)。
pub struct Shared<T> {
    head: Option<Rc<RcNode<T>>>,
}

pub struct RcNode<T> {
    elem: T,
    next: Option<Rc<RcNode<T>>>,
}

impl<T> Shared<T> {
    pub fn new() -> Self {
        Shared { head: None }
    }

    /// 注意签名是 &self 不是 &mut self——不修改旧版本,造一个新版本(持久化语义)。
    /// Rc::clone 只是计数 +1,零深拷贝。
    pub fn prepend(&self, elem: T) -> Shared<T> {
        todo!("L5X prepend:新节点的 next 是 self.head.clone(),包成新 Shared")
    }

    /// 去掉头元素的视图——同样不动原列表。
    pub fn tail(&self) -> Shared<T> {
        Shared {
            head: self.head.as_ref().and_then(|n| n.next.clone()),
        }
    }

    pub fn head(&self) -> Option<&T> {
        self.head.as_deref().map(|n| &n.elem)
    }

    /// 头节点的强引用计数——"这段历史还有几个人记得"。
    pub fn strong_count(&self) -> usize {
        self.head.as_ref().map(Rc::strong_count).unwrap_or(0)
    }
}

impl<T> Default for Shared<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_lifo() {
        let mut h = History::new();
        h.push("boot");
        h.push("login");
        h.push("crash");
        assert_eq!(h.len(), 3);
        assert_eq!(h.pop(), Some("crash")); // 后进先出
        assert_eq!(h.pop(), Some("login"));
        assert_eq!(h.pop(), Some("boot"));
        assert_eq!(h.pop(), None); // 空了返回 None,不 panic
    }

    #[test]
    fn peek_borrows_not_takes() {
        let mut h = History::new();
        h.push(String::from("evt"));
        assert_eq!(h.peek(), Some(&String::from("evt")));
        assert_eq!(h.len(), 1); // peek 之后还在——只是借用
        if let Some(top) = h.peek_mut() {
            top.push('!');
        }
        assert_eq!(h.pop(), Some(String::from("evt!")));
    }

    #[test]
    fn iter_zero_copy() {
        let mut h = History::new();
        h.push(1);
        h.push(2);
        h.push(3);
        let v: Vec<&i32> = h.iter().collect();
        assert_eq!(v, vec![&3, &2, &1]);
        assert_eq!(h.len(), 3); // 遍历不消耗
    }

    /// AI 常忘写迭代式 Drop——它的版本能过前面所有测试,在这里爆栈。
    #[test]
    fn long_chain_drop_no_overflow() {
        let mut h = History::new();
        for i in 0..200_000 {
            h.push(i);
        }
        drop(h); // 递归 Drop 会在这里栈溢出
    }

    #[test]
    fn into_iter_consumes() {
        let mut h = History::new();
        h.push(1);
        h.push(2);
        h.push(3);
        let v: Vec<i32> = h.into_iter().collect(); // h 被吃掉,元素所有权交出
        assert_eq!(v, vec![3, 2, 1]);
        let mut h2 = History::new();
        h2.push(String::from("owned"));
        for s in h2 {
            // 实现了 IntoIterator → for 直接吃 History,拿到的是 String 本体
            let _owned: String = s;
        }
    }

    #[test]
    fn iter_mut_edits_in_place() {
        let mut h = History::new();
        h.push(1);
        h.push(2);
        h.push(3);
        for x in h.iter_mut() {
            *x *= 10; // 原地改,零拷贝
        }
        let v: Vec<&i32> = h.iter().collect();
        assert_eq!(v, vec![&30, &20, &10]);
    }

    #[test]
    fn shared_tail_between_versions() {
        let base = Shared::new().prepend("boot").prepend("login");
        let a = base.prepend("open_a"); // 分叉!
        let b = base.prepend("open_b"); // 再分叉!
        assert_eq!(a.head(), Some(&"open_a"));
        assert_eq!(b.head(), Some(&"open_b"));
        assert_eq!(a.tail().head(), Some(&"login")); // 尾巴是同一条
        assert_eq!(base.strong_count(), 3); // "login" 被 base/a/b 三方共享
        drop(a);
        drop(b);
        assert_eq!(base.strong_count(), 1); // 分支还回去,计数退潮
    }
}
