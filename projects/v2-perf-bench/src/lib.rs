//! 第 5 课配套：**同一功能的"朴素版 vs 优化版"两套实现**，用于演示
//! "先测量后优化、不要猜"的性能工作流。
//!
//! 角色说明（为什么是 lib + bench 而不是 bin）：
//! - 两版实现**结果等价**——由 `#[test]` 保证**正确性**；
//! - 性能差异由 `benches/` 用 **criterion + `black_box`** 测量——测试管对错，
//!   基准管快慢，二者分工不同，不要用 `Instant::now()` 手测来下结论。
//!
//! 本 crate 给出两组对照：
//! 1. **词频统计**：
//!    - 朴素版 [`word_count_naive`]：到处 `String` 分配 + `Vec` 线性查找（O(n·m)）；
//!    - 优化版 [`word_count_fast`]：`&str` 切片借用输入（零额外分配）+ `HashMap::entry`
//!      + `with_capacity` 预分配（O(n)）。
//! 2. **平方和**：
//!    - 顺序版 [`sum_squares_sequential`]：单线程迭代器；
//!    - 并行版 [`sum_squares_parallel`]：`rayon` 数据并行，自动切分到多核。
//!
//! 赏析锚点：`ripgrep`——真实的性能工程把"避免分配 / SIMD 加速的 memchr /
//! 并行目录遍历"组合起来，本课的两组对照是它的"显微镜版"。

use std::collections::HashMap;

/// 朴素版词频统计：用 `Vec<(String, usize)>` 当累加器。
///
/// 故意写得"不地道"，以便基准对照：
/// - 每个词都 `to_string()`——**每词一次堆分配**；
/// - 用 `Vec` 线性查找已有词——**整体复杂度 O(n·m)**（n 个词、m 个不同词）。
///
/// 返回 `Vec<(词, 次数)>`，顺序为首次出现顺序。
#[must_use]
pub fn word_count_naive(text: &str) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for word in text.split_whitespace() {
        // 朴素写法的代价：为每个词都分配一个 String，哪怕它已经存在。
        let owned = word.to_string();
        let mut hit = false;
        for slot in &mut counts {
            if slot.0 == owned {
                slot.1 += 1;
                hit = true;
                break;
            }
        }
        if !hit {
            counts.push((owned, 1));
        }
    }
    counts
}

/// 优化版词频统计：`HashMap<&str, usize>` 直接**借用输入切片**。
///
/// 三处优化：
/// - 键是 `&str`，复用输入内存——**统计过程中零额外堆分配**；
/// - `entry(..).or_insert(0)`——一次哈希查找完成"查 + 插 + 改"；
/// - `with_capacity` 预估容量——减少哈希表 rehash。
///
/// 返回的 `HashMap` 借用了 `text`，因此生命周期 `'a` 与输入绑定。
#[must_use]
pub fn word_count_fast(text: &str) -> HashMap<&str, usize> {
    // 容量是"粗上界"启发式：按字节数估词数（假设平均词长约 8 字节，通常会高估），
    // 不必精确——目的只是减少扩容次数。
    let mut counts: HashMap<&str, usize> = HashMap::with_capacity(text.len() / 8 + 1);
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

/// 把任意一版的结果归一化成"按词排序的 `Vec`"，便于跨实现比较相等。
///
/// 公开它是为了让基准与调用方都能把两版结果摆到同一基准上验证等价。
#[must_use]
pub fn canonicalize<'a, I>(pairs: I) -> Vec<(String, usize)>
where
    I: IntoIterator<Item = (&'a str, usize)>,
{
    let mut v: Vec<(String, usize)> = pairs.into_iter().map(|(w, c)| (w.to_string(), c)).collect();
    v.sort();
    v
}

/// 顺序版：单线程求各元素平方之和（用 `wrapping_*` 保证可结合、不会 panic 溢出）。
#[must_use]
pub fn sum_squares_sequential(values: &[u64]) -> u64 {
    values
        .iter()
        .fold(0u64, |acc, &x| acc.wrapping_add(x.wrapping_mul(x)))
}

/// 并行版：用 `rayon` 把同样的平方和切分到多核归约。
///
/// `wrapping_add` 满足结合律与交换律，因此并行归约的结果**与顺序版逐位相等**——
/// 这是"并行优化不能改变结果"的前提，也是 `#[test]` 要守住的不变量。
#[must_use]
pub fn sum_squares_parallel(values: &[u64]) -> u64 {
    use rayon::prelude::*;
    values
        .par_iter()
        .map(|&x| x.wrapping_mul(x))
        .reduce(|| 0u64, u64::wrapping_add)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "the quick brown fox the lazy dog the fox";

    #[test]
    fn naive_and_fast_agree() {
        let naive_pairs = word_count_naive(SAMPLE);
        let naive = canonicalize(naive_pairs.iter().map(|(w, c)| (w.as_str(), *c)));
        let fast = canonicalize(word_count_fast(SAMPLE));
        // 正确性核心：两版实现对同一输入产出完全相同的词频。
        assert_eq!(naive, fast);
    }

    #[test]
    fn word_counts_are_correct() {
        let map = word_count_fast(SAMPLE);
        assert_eq!(map.get("the"), Some(&3));
        assert_eq!(map.get("fox"), Some(&2));
        assert_eq!(map.get("quick"), Some(&1));
        assert_eq!(map.get("missing"), None);
        // 6 个不同的词：the/quick/brown/fox/lazy/dog。
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn empty_and_whitespace_input() {
        assert!(word_count_naive("").is_empty());
        assert!(word_count_fast("   \n\t  ").is_empty());
        assert_eq!(sum_squares_sequential(&[]), 0);
        assert_eq!(sum_squares_parallel(&[]), 0);
    }

    #[test]
    fn sequential_and_parallel_agree() {
        let values: Vec<u64> = (0..10_000).collect();
        let seq = sum_squares_sequential(&values);
        let par = sum_squares_parallel(&values);
        // 并行版必须与顺序版逐位相等（wrapping_add 可结合）。
        assert_eq!(seq, par);
        // 顺手核对一个已知闭式：sum_{0..n} i^2 = (n-1)n(2n-1)/6。
        let n = 10_000u64;
        let expected = (n - 1) * n * (2 * n - 1) / 6;
        assert_eq!(seq, expected);
    }
}
