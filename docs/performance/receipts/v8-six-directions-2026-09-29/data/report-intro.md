# 六个 V8-v7 优先子项：Profile、并行候选与验收

## 目标与范围

主目标是在当前源码基线 `7723e825` 上降低固定工作量的完整进程时间，优先
Earley-Boyer、RayTrace、Richards、Crypto、DeltaBlue、Splay。保留上游 workload
主体、断言、JavaScript 可观察顺序、完整代际身份、所有权、safe Rust，以及冻结
Test262 admission/result body。允许小幅代码增长；通过其他两项 V8、八项合并程序
和 RealWorld 应用检查交换。新发现的重复 shape 反向边另以存储增长上界验收。

用户给出的 QuickJS Score / m0 Score（28.0×、27.0×、23.1×、18.0×、17.0×、
14.8×）用于优先级，未重新确认其历史构建身份。本报告不把固定工作量进程耗时
冒充 adaptive V8 Score，也不把本轮收益直接从这些历史倍数中相减。

基线和最终候选 `ee999771` 都用 Rust 1.88.0、普通 release、fat LTO、CGU=1
重新构建，profiling 另建。实现并行推进，正式时间/硬件/RSS 采样串行；采样期间
本任务构建、测试和其他 agent 负载停止。CPU 2，ABBA/BAAB，fresh process，逐次
校验源码/二进制 hash、退出码、完整 stdout 和空 stderr。桌面后台程序仍存在，
没有声称整机隔离。全部有效原始样本和不利结果保留。初期 /tmp 容量不足导致的构建失败未进入
计时，构建目录已迁往磁盘后重跑。

## 现状证据与机制选择

八项均重新收集逻辑 Profile；六项另收 `cycles:u` 探索样本。后者用代码与当前
基线仅差文档注释的上一轮普通二进制，在收逻辑 Profile 时进行，样本数有限，
只用于定位，不能当正式时间、收益上界或各候选收益之和。验收使用本轮普通构建。

| 子项 | 当前证据 | 本轮改变及限制 |
| --- | --- | --- |
| Earley-Boyer | 512,379 call、267,379 construct、534,555 missing committed；`sc_Pair` 频繁构造 | 追加属性保留旧槽 owners；scalar write 实际命中为零，不能声称它改善本项 |
| RayTrace | 66,600 construct、235,680 set，其中 194,985 missing committed | shape 追加和已有数值字段写入分别测；大量 miss 会承担新快路径检查成本 |
| Richards | 50,751 set，只有 9 个 set state；主要是同步写入交接与协议成本 | own scalar 字段在 execute 内提交，不归因于 continuation 分配 |
| Crypto | 12,534,392 in-place number binary；`am3` 是高频 guest 执行函数 | Int ToInt32 实验被拒绝；另试 operand push 冷错误提取，现有数组优化不重复计收益 |
| DeltaBlue | 115,452 call、22,662 set；大量字段读和引用流动 | existing scalar 写覆盖部分工作，未为所有调用增加新协议 |
| Splay | 626,943 call；Profile 指向 layout edges、retain 和 shape cache 清理 | 保留旧槽 owners，限制重复反向边；现有帧池已经大量复用，未再做 pooling |

逻辑事件和 guest PC visits 不是 CPU 时间占比。函数/PC 原始记录和 omission 字段
随 Profile 一同保存，探索采样的低样本热点也完整保留。

五项候选分别基于同一基线实现并独立构建，其中整数实验已拒绝：

1. **整数事实直接消费（拒绝）**（`03046503`）：`Number::Int(i32)` 无需转换为 f64 后
   截断/取模。现有 kernel 接口允许直接完成；Float 分支保持原算法。实际普通
   二进制机器码确认旧 Int 路径调用 trunc/fmod，新路径跳过这些调用。相关函数
   自身体积增加 327 bytes，不能把删除运算等同于所有代码都变小。
2. **已有普通标量字段本地写入**（`3cb5fd54`）：现有 execute 可本地完成，增加
   局部 helper。只准入 ordinary、own writable data、旧/新值均无引用边、receiver
   释放不会触发 cleanup 的事务。未准入时保持两个 operands；成功递增 property_generation 并
   消费原 owners。引用槽、缺失、只读、accessor、Proxy、数组等沿原合同执行。
3. **canonical shape 追加保留旧 owners**（生产补丁 `775a4f9e`，独立最终
   `c1917034`）：原完整布局替换接口接收完整 Slots，并重复 retain/release 全布局。
   新内部 append 事务表达旧槽不变，仅准备新增 slot 和 successor shape 的边，
   发布后释放旧 shape。它保留完整前缀、prototype、storage 与代际验证；准备失败
   回滚新 owners，已发布错误不撤销新槽 Atom 所有权。前缀检查仍 O(width)，没有
   宣称属性创建整体 O(1)。常见 new shape + one data edge 还复用已有的小边列表
   preflight，避免旧全布局边较多时的 HashMap 聚合。底层 `Slots::try_reserve`
   仍会重建 spilled Vec 并复制旧槽；本轮没有消除全部复制或分配。
4. **反向 shape 边唯一性**（`bf5d9b88`）：命中同一前向边时不重复 push 反向记录。
   利用已有 HashMap insert 的 previous 值，重复路径不新增线性去重扫描。重绑定
   时先摘除旧代际反向边，避免旧目标延迟 cleanup 删除新 forward edge。无新通用
   cache 框架；反向记录条目数由不同关系决定，不再随同边重复使用增长；容量保留历史高水位。

5. **operand push 冷错误提取**（`e635b49d`）：仅把两个错误构造移到 cold、
   inline(never) helper，保留检查顺序、错误文本和 pending owner 消费时机。原有
   inline 标记因此生效，实际普通机器码中独立 `operand_push_index` 消失，
   `commit_push`/`FrameSlots::push` 成功路径不再调用 helper，重复 bounds 合并为
   一次。不是删除容量/空槽校验，也未增加 VM 模式。独立 .text 增 400 bytes，
   完整 ELF 增 184 bytes；47 项已有栈测试通过。

独立静态审查未发现可确认缺陷；这不能替代动态测试或 OOM 注入。定向测试覆盖
饱和旧引用不被再次 retain、Symbol 回滚、失效对象/shape、完整前缀、相同 getter/
setter 双边、自引用、删除重增和原型缓存。重复边测试执行 10,000 次后长度仍为 1、
容量等于第一次，且不多留 shape 强引用。

## 调研与未采用路线

[QuickJS 内部实现说明](https://bellard.org/quickjs/quickjs.pdf)提供 shared shapes、
atoms 和引用计数的背景；[V8 属性实现说明](https://v8.dev/blog/fast-properties)
说明命名属性、元素和 shape transition 的分工。这些资料只提供候选机制，优先级
与本轮收益均由本仓库实际执行证据确定，不直接套用 JIT 的结论。

- 空闭包 owner 删除已作为小实验隔离，但本轮未采用；现有调用 cache、参数转移
  和帧池已经生效，shape 追加证据更直接。
- 命名读的 cache miss 值得继续区分：Richards hit/miss 104,019/97,905，DeltaBlue
  187,874/146,074，RayTrace 470,894/502,269。应区分 shape 多样性、短命 shape 代际
  和真正原型修改；普通新对象变布局不会使全局原型 epoch 失效。直接增大 PIC
  容量尚无足够证据，也会增加不参与路径的存储成本。
- Crypto 的 operand push/commit 已做最小实验；剩余索引协议可继续区分。扩大 VM 融合、
  重写 Float ToInt32 或改变数值表示需要新的区分实验，本轮没有为此增加指令模式。
- 不把“没有明显收益”自动解释成覆盖不足；各独立结果与采用理由见下文。

后续已确认的局部机会：`Slots::try_reserve` 对 spilled Vec 仍分配新 Vec 并复制
旧槽，即使可能保有可用容量。可独立测试原 Vec reserve，先保留分配失败合同与
inline→spilled 转换，再测 Splay/Earley 和内存高水位。本轮未实现，未计入收益。
