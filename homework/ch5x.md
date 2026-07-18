# L5+ 加练 · 事件历史链表(模块一综合)

> 对应教材 L5+「用链表打通所有权与类型系统」。代码骨架在 `stages/L5X-listlab/`。
> **PR 标题格式:`[hw5x] 你的姓名或ID`**(CI 按此只跑本课门禁)。

## 需求

实现两个数据结构(全程**零 unsafe、零 unwrap(测试除外)、零深拷贝 clone**):

1. `History<T>`(Box 单链表,undo 栈):补全 `push` / `pop` / `peek` / `Iter::next` / 迭代式 `Drop` 五处 `todo!()`;
2. `Shared<T>`(Rc 持久化链表,历史分叉):补全 `prepend`。

## 协作方式

1. **先画图**:push/pop 前后的所有权链、Rc 分叉的共享尾巴(教材图 40/41);
2. 让 AI 写,逐行审查它的 `clone` 和 `unwrap`;
3. 跑测试验证你的审查。

## 验收标准

- `cargo test -p winmon-listlab` **5 个测试全绿**——特别注意 `long_chain_drop_no_overflow`(20 万节点):AI 忘写迭代式 Drop 时它会爆栈,而其余 4 个照样绿;
- **PR 三问**(D8 评分项):AI 在哪里用 clone 绕所有权移交?它的 Drop 能过长链测试吗?`prepend` 它写的是 `&self` 还是 `&mut self`?

## 提示

- 撞 E0507 时想想 `Option::take`(先补位再拿);
- `Iter<'a, T>` 是临时视图结构体,`'a` 约束它不能活得比 `History` 久(L3+ §1.4);
- `Rc::clone` 是计数 +1 不是深拷贝——它不违反零拷贝红线,而且是 `prepend` 的正解。
