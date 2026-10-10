# 原生检查点：oxide vs brimstone（2026-10-11）

应用户决定：检查点不再对照 Boa，外部参照改为 **brimstone**
（[Hans-Halverson/brimstone](https://github.com/Hans-Halverson/brimstone)，
固定提交 `697465b08fc820a64d25f0db226468842bdc907e`，按其 rust-toolchain 用
rustc 1.95.0 构建，release）。

## 方法

- 参考测量机：AMD Ryzen 7 7840HS，governor=performance，boost 关；`run.py
  --suite v8-v7 --cpu 2` 钉核；V8-v7 固定 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。
- oxide 候选：`ab382bed`（PR #107 头），`build.py` 回执构建，**rustc 1.95.0**
  （用户决定：oxide 迁 1.95，测量先行），LTO fat / codegen-units=1。
- 设计：九用例（八独立 + `all` 合并）× 两引擎 × repeat 8 × abba-baab；
  同二进制 A/A 先行（oxide/oxide-aa）冻结噪声。
- 原始证据：`/tmp/oxide-native-aa`、`/tmp/oxide-native-ab`（samples.jsonl +
  results.json + paired.json）；机器测量期间有 qemu 虚拟机与 webkit 进程在跑
  （~17% CPU），单项 A/A 噪声 ≤1.8%，对本对比的量级无影响。

## 结果（原版 Score，越高越好；gain = oxide 相对 brimstone）

| 用例 | brimstone | oxide | oxide gain (95% CI) |
|---|---:|---:|---:|
| richards | 1477.0 | 165.5 | −88.8% [−89.0, −88.7] |
| deltablue | 1813.0 | 156.0 | −91.4% [−91.7, −91.3] |
| crypto | 1431.5 | 248.5 | −82.6% [−83.2, −82.2] |
| raytrace | 2639.0 | 350.0 | −86.7% [−86.9, −86.5] |
| earley-boyer | 3743.5 | 388.0 | −89.6% [−89.7, −89.6] |
| regexp | 2728.0 | 342.0 | −87.5% [−87.7, −87.4] |
| splay | 6865.5 | 963.0 | −86.0% [−86.1, −85.7] |
| navier-stokes | 2230.5 | 665.5 | −70.1% [−70.5, −69.7] |
| **all（合并）** | **2551.5** | **342.0** | **−86.6% [−86.7, −86.5]** |

合并子项与独立用例一致（Richards −89.0% 等，见 paired.json
`combined_subscores`）。每用例 8 对配对样本，CI 极窄（±0.5%）。

## 读法

- brimstone 在本套件上比 oxide 快 **5.7×（navier）至 11.6×（deltablue）**，
  合并约 **7.5×**。参照系：oxide 对 Boa 在 #104 时代约为 106.7%（近平），
  即 brimstone 合并分约为同期 Boa 的 9–10×。
- 差距结构与 5e-1/5e-2 的 Ir 取证一致：oxide 的成本主体在解释器分派与
  arm 守卫（栈式 VM、逐操作数所有者检查）；brimstone 是寄存器 VM +
  内联缓存 + 紧凑 GC。 oxide 的 Ir 路线（7 系列 arm 工作优化）方向正确
  但量级不在同一档；进一步数量级差距需要结构性改变（寄存器化/IC 化），
  超出第 5 项范围。
- 本对比不构成任何验收门槛（noninferior 门是同引擎回归用的）；它是
  阶段 B + 第 5 项收口后的外部坐标。

## 公平性核查附录（2026-10-11，应质疑补做）

质疑：zoo.js.org（[JavaScript Engines Zoo](https://zoo.js.org/)，
[ivankra/javascript-zoo](https://github.com/ivankra/javascript-zoo)）的 Octane
数据显示 brimstone ≈ boa，与本检查点的 7.5× 矛盾。核查结论：**测量公平，
矛盾来自套件差异与参照系误读**。

1. **同字节同机器**：两侧执行同一 `dist/quickjs-oxide` 产物
   （workload_sha256 一致），CPU 2 钉核，同一进程内 ≥1 秒 / ≥32 迭代的真实
   计时（base.js `RunSingleBenchmark` 读码确认；Score = 固定 reference /
   每迭代微秒，跨引擎比分即时间比）。两侧 wall time 相近（~2s/用例）是
   ≥1s 计时底噪的设计产物，不代表吞吐相等。
2. **独立交叉验证（决定性）**：2026-10-01 同机同套件测过 C QuickJS 与 Boa
   （~/.cache/oxide-history-three-engines-2026-10-01）。brimstone 与
   quickjs 逐项比：richards 1.09×、deltablue 1.49×、crypto 1.02×、
   raytrace 1.01×、earley-boyer 1.23×、regexp 4.36×（brimstone 自带 regexp
   引擎占优）、splay 1.40×、navier 0.76×。**brimstone ≈ quickjs 级**
   （0.76–1.49× 逐案散布，两个独立引擎的典型形态）——测量无作弊迹象。
3. **真实头条被参照系掩盖**：本套件上 quickjs 本身就是 1216–4907 分、
   Boa 只有 206–833（quickjs ≈ 5–6× Boa）；oxide 165–963，**比
   quickjs 级慢 ~8×**（10-01 时为 ~14×，阶段 B + 第 5 项已收窄）、在
   richards/deltablue 独立用例上约为 Boa 的 0.75×。oxide 的差距与全部
   Ir 取证一致（解释器 arm 工作主导），不是测量假象。
4. **zoo 印象的来源**：zoo 跑 Octane（Box2D/CodeLoad/Mandreel/PdfJS 等大
   程序 + 启动/GC 主导），其 brimstone 构建为 2025-12 修订（旧 10 个月）；
   Octane 上 brimstone ≈ boa 与 V8-v7 上 brimstone ≈ quickjs ≫ boa 可以
   同时成立——两个套件测的是不同东西（解释器热循环 vs 大程序综合）。
5. 旁证：[HN yt-dlp 实测](https://news.ycombinator.com/item?id=45898407)
   quickjs 2.3s / brimstone 6.3s / boa 88s ——真实世界代码上 quickjs 级
   最快，与本文 V8-v7 排序不冲突。
