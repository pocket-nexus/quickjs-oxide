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
