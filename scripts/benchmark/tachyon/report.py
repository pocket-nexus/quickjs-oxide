"""Write the architecture-focused report from validated experiment artifacts."""
import json
from pathlib import Path

ROOT=Path.cwd(); OUT=ROOT/'target/tachyon-comparison'
s=json.loads((OUT/'summary.json').read_text())
T='https://github.com/tachyon-engine/tachyon-engine/blob/2d148e462233c884d0547d4ec0ccc8ccaa183f17/'
Q='https://github.com/bellard/quickjs/blob/3d5e064e9dd67c70f7962836505a7fa067bf0a4e/'
O='https://github.com/pocket-stack/quickjs-oxide/blob/1cc51bb5fcc5c36912d3197d877219ae513dc4b5/'
def link(base,path,line,label=None):return f'[{label or path}]({base}{path}#L{line})'
def vm(path,line):return link(T,'crates/tachyon-vm/src/'+path,line)
lines=['# Tachyon 架构与 VM：我们应该吸收什么','',
'本次将 Tachyon 加入 #16 的同机 benchmark，并将研究重点收敛为架构与 VM 设计。**最值得吸收的是：显式执行状态、与运行时分离的不可变代码、以及有严格边界的热执行内核。** 这比照着另一引擎的局部快路径逐项改写更有长期价值。','',
'结论分三层：代码和小实验已经确认的能力；相对 QuickJS 在特定目标上的结构优势；仍需在 Oxide 上验证的演进方案。这里不分析 Tachyon 的性能短板，也不把微基准优势外推为整体优于 QuickJS。','',
'## 1. 显式 Fiber / frame / continuation：最重要的架构参照','',
'Tachyon 将 JS 调用状态放在 `Fiber.frames: Vec<Frame>` 和寄存器存储中。Frame 明确记录 code、function、pc、base、this、arguments、handler/completion 边界，编译期断言大小为 104 bytes。函数调用入帧、返回和需要执行 JS 的内置操作，通过解释器及 continuation 推进。',
'价值不是仅仅“减少一次函数调用”，而是让 JS 执行状态成为 VM 可管理的数据：暂停、恢复、预算调度、异常回退和 GC root 枚举可以围绕同一套状态组织。内置 String 转换、Array 回调等也能把跨 JS 调用的状态放进受 GC 管理的 continuation，避免状态只存在于宿主语言调用栈。',
f'源码：{vm("runtime/fiber.rs",4152)}、{vm("interpreter.rs",8603)}、{vm("string_concat.rs",61)}。',
f'QuickJS 对照：普通 JS 调用使用 C 局部 `JSStackFrame`、`alloca` 的局部/操作数区，以及递归 `JS_CallInternal`；异步函数和 generator 另有持久状态。见 {link(Q,"quickjs.c",17772)}、{link(Q,"quickjs.c",18217)}。因此，在**需要让任意深度的内部 JS 调用状态脱离宿主栈、统一调度**的设计目标上，Tachyon 的组织方式更适合作为参照；这不意味着无限递归或没有资源限制。',
'对 Oxide：把宿主栈预算问题与普通 JS frame 的表示分开处理。先设计显式 frame/continuation 的所有权、return target、异常和 reentry 规则；不用先改 GC 或整套字节码。','',
'## 2. 可恢复执行预算 + executor-neutral Future：已有可运行证据','',
'Tachyon 区分 fuel 与 quantum；有界路径按字节码消耗预算，MAX/MAX 路径则选择编译期消除预算检查的实例。`VmDriver` 把 isolate 内的模块/Promise 工作推进映射为 Rust `Future::poll`，执行状态保留在 isolate，而非临时 Future 对象里。',
'本次编写并实际运行的架构探针：',
'- 创建一个 Promise job，在回调中执行 10,000 次求和循环；quantum 设为 8。',
'- 前 5 次 poll 后销毁宿主 Future，再为同一 Promise 创建新的 Future。',
'- 总计 **10,003 次 poll** 后得到正确结果 **49,995,000**。任务没有因 Future 重建丢失或从头执行。',
f'源码：{vm("driver.rs",45)}、{vm("driver.rs",101)}、{vm("interpreter.rs",654)}。',
f'QuickJS 对照：`JS_SetInterruptHandler` 提供中断回调，`JS_ExecutePendingJob` 推进任务；中断返回失败进入异常，并非同一种 quantum 用尽后继续运行的 Future 协议。见 {link(Q,"quickjs.c",2236)}、{link(Q,"quickjs.c",2303)}、{link(Q,"quickjs.c",7855)}。',
'对 Rust 服务内嵌脚本、多租户公平性、合作式取消，这种**可恢复的调度契约**比单纯设置 timeout 更有价值。实验覆盖 Promise job 的字节码执行；不能将它扩张为原生内置循环/GC 的硬实时保证，也不等同于现成的任意顶层 Script 公共 resume API。','',
'## 3. 热内核与慢路径之间有可证明的边界，而非分散的 fast path','',
'Tachyon 每个批次先取得已验证 bytecode cursor 与 register window，PC 留在局部变量中。热内核只执行不分配、不调用 JS、不改变寄存器 backing 的操作。遇到慢操作时先发布 PC，退出这段存储不变的执行区间，再执行完整语义逻辑并重新建立窗口。',
f'核心实现：{vm("interpreter.rs",692)}、{vm("interpreter.rs",10319)}、{vm("runtime/code.rs",35)}。默认批次为 8：{vm("tuning/dispatch.rs",1)}。',
'本次做了一个因果性更强的对照：只改 **DEFAULT_DISPATCH_BATCH: 8 → 1**，编译器、release 设置、脚本和 host 全相同；每项五轮交替运行。','',
'| 负载 | batch=8 ms | batch=1 ms | batch=1 / batch=8 |','|---|---:|---:|---:|']
for case,es in s['batch'].items():
 a,b=es['batch8']['median_ms'],es['batch1']['median_ms']
 lines.append(f'| {case} | {a:.2f} | {b:.2f} | {b/a:.2f}× |')
lines += ['',
'这说明批次内保存执行状态的收益在这些负载上确实存在；不是只根据两个不同引擎的跑分猜原因。它并不能单独解释全部跨引擎差距。',
f'QuickJS 本身已经把 pc/sp 等放在 C 局部变量中，并支持 computed-goto 分派，见 {link(Q,"quickjs.c",17772)}。所以这里应学习的是 **Rust 中“验证 → 稳定窗口 → 热内核 → 慢路径发布/重建”的结构**，不能把“8 条一批”当成优于 QuickJS 的通用魔法。',
'对 Oxide：这是最适合先做受控原型的 VM 优化。保留现有语义入口，把 Number/Bool/局部值操作集中为一段没有可观察调用的执行区间，减少重复状态取用与回写。项目现有 `unsafe_code = forbid` 应保持；先探索安全 Rust 中的窗口和借用组织，不直接复制其 unchecked 访问。','',
'## 4. 已验证的不可变代码：验证结果成为执行接口的一部分','',
'Tachyon 的 `Bytecode → VerifiedBytecode → CompiledModule` 边界检查寄存器、常量、调用参数窗口、跳转到合法指令起点、函数布局及其他元数据；代码由不可变 Arc backing 持有。VM 热 decoder 明确依赖这些不变量，而非每条指令重新完成同一套结构验证。',
f'源码：{link(T,"crates/tachyon-bytecode/src/lib.rs",39)}、{link(T,"crates/tachyon-bytecode/src/verify.rs",10)}。本次探针确认畸形 opcode 流在验证入口被拒绝。',
'QuickJS 官方文档明确要求不从不可信来源加载其序列化字节码。Tachyon 的结构验证边界因而是一个值得借鉴的设计点；单个拒绝探针不是完整安全审计，也不构成可以安全执行任意不可信序列化格式的承诺。[QuickJS 文档](https://bellard.org/quickjs/quickjs.html#Script-evaluation)',
f'**我们已经有相近基础**：{link(O,"src/engine/code/bytecode_publish/verified.rs",8,"Oxide VerifiedFunction")} 和发布事务拥有验证后的草稿，防止验证后替换。下一步的价值是让执行层更充分使用已经建立的不变量，而不是重新造一遍校验器。','',
'## 5. 代码与 Isolate 绑定分离：跨隔离堆共享编译产物','',
'`CompiledModule` 保存 Arc 源码、常量描述、scope names 和函数；`LoadedCode` 则保存某个 isolate/realm 的 code identity、scope resolutions 与已物化 constant values。编译产物和运行时绑定是两个不同对象。',
f'源码：{link(T,"crates/tachyon-bytecode/src/lib.rs",705)}、{vm("runtime/code.rs",27)}。',
'本次实验确认：`CompiledModule: Send + Sync` 可通过编译；同一 module 在 Isolate A、B、A 执行计数脚本，依次返回 **1、1、2**。代码共享，两个全局环境保持独立。',
f'QuickJS 的函数字节码持有 realm 指针，见 {link(Q,"quickjs.c",685)}。跨独立 runtime 复用通常要处理序列化/重新加载与 runtime 绑定；同 runtime 的多个 context 可以共享对象，不应与跨隔离堆代码共享混为一谈。',
'这是服务器多 isolate、代码缓存和并行编译场景中，很可能比紧耦合运行时对象的方案更合适的结构。对 Oxide，可以评估将“可共享纯代码/常量描述”和“本地 atom、heap value、闭包环境、缓存”分层；先以重复加载同一程序的内存和初始化成本验收。','',
'## 6. 64-bit 平台上的紧凑 Value：可直接确认的表示优势','',
'在本机 x86-64 上实际编译检查：**Tachyon Value = 8 bytes；QuickJS JSValue = 16 bytes**。Tachyon 使用 NaN boxing 和 32-bit logical heap reference；QuickJS 此平台使用 payload + tag 的结构体。',
f'源码：{link(T,"crates/tachyon-value/src/lib.rs",196)}、{link(T,"crates/tachyon-gc/src/layout.rs",8)}、{link(Q,"quickjs.h",216)}。',
'这个优势指值数组、寄存器与 frame 中 Value 字段的密度：64-byte cache line 理论上可放 8 个而非 4 个 Value。整体内存并不因此自动减半；logical heap address、外部 backing、root 和 GC 元数据仍有自己的成本和约束。',
'对 Oxide：可以先量化 RawValue 在 operand/local/argument 存储中的尺寸和复制成本，评估“紧凑执行值 + 旁表/heap backing”作为独立架构方案。它与预算/热内核可以分别推进，不必打包成一次 GC 重写。','',
'## 7. Root / NoGcScope / write barrier：把 GC 边界写进 API','',
'Tachyon 的 `Local` 使用 generative lifetime 且不可跨线程传递；`RunningScope` 管理 root checkpoint；`NoGcScope` 暂时移除分配/收集 API，使借用 payload 的阶段和可能触发 GC 的阶段分开。write barrier、持久 root 和外部 backing 记账也有明确入口。',
f'源码：{link(T,"crates/tachyon-gc/src/scope.rs",26)}、{link(T,"crates/tachyon-gc/src/scope.rs",241)}、{link(T,"crates/tachyon-gc/src/heap.rs",458)}。',
'最值得吸收的是“可分配/可观察”和“可借用/不可移动”的边界组织方式。它让热内核何时必须退出、临时 root 何时有效、跨调用状态由谁持有更容易表达与测试。QuickJS 的 RC+cycle-removal 有不同取舍；这里不能由 API 形式推出 tracing GC 必然更快。',
'对 Oxide：保留当前生命周期模型，先提炼内部 borrowed execution epoch 与拥有状态的 slow-path continuation，减少为跨层调用而反复 clone/retain/release 的需要。','',
'## 8. 前端、运行时与宿主能力分层：为演进留出空间','',
'Tachyon 把 Oxc 解析、owned HIR、register bytecode、VM 和 host providers 分开。register 指令显式编码来源/目的寄存器，函数布局在运行前确定；宿主提供时钟、熵、时区、Intl、任务驱动等能力，核心不隐式读取文件或依赖特定 executor。',
f'源码：{link(T,"crates/tachyon-compiler/src/lib.rs",82)}、{link(T,"crates/tachyon-compiler/src/bytecode.rs",23)}、{link(T,"crates/tachyon-bytecode/src/encoding.rs",90)}、{vm("host.rs",1311)}。',
'这为后续局部数据流优化、独立编译、确定性宿主测试提供了清晰边界。它并不证明 register VM 必然优于 QuickJS 的紧凑栈字节码，也不代表已经具备成熟 SSA/JIT。',
f'Oxide 已有 {link(O,"src/engine/host/mod.rs",15,"HostServices")} 和独立适配层；可借鉴的是把能力契约、暂停状态和编译结果进一步显式化，保留已有分层成果。','',
'## 对我们下一阶段的建议','',
'按架构收益与可分步验证的程度，建议做三个独立原型：',
'1. **显式执行状态与退出原因**：梳理 frame、PC、operand/local、return target、异常和 reentry 状态，设计 Completed/Thrown/Yielded 的内部边界。用深调用、异常/重入与暂停恢复验证所有权；不先扩大宿主栈预算。',
'2. **验证后的原语执行批次**：保留现有栈字节码，先为不分配、不调用 JS 的指令建立热内核；在完整语义边界发布状态并重新绑定。用 batch=1/4/8、硬件指令数和调用密集负载验证，而非只看单个空循环。',
'3. **共享纯代码 + 本地运行时绑定**：抽出不含 runtime roots 的不可变代码，验证跨 isolate 重用、常量物化隔离和失效规则。若支持 Promise/模块 quantum 驱动，再测多任务公平性。',
'紧凑 Value、分代 GC 和完整 register VM 作为随后单独评估的架构议题。静态 Atom 等局部优化仍可做，但不应成为这次 Tachyon 架构研究的主线。','',
'## 用于支持架构判断的测量证据','',
'已完成 58 个固定负载 × 3 引擎 × 5 轮 = **870** 个输出通过的进程测量。另有匹配构建设置的 **75** 个样本、batch 8/1 的 **50** 个样本，以及聚焦 8 项的 **24** 次 profile / **72** 次硬件计数。最终选用的 profile 无 lost samples。',
'三引擎固定基线是 Tachyon `2d148e462233c884d0547d4ec0ccc8ccaa183f17`、Oxide PR19 引擎 `1cc51bb5fcc5c36912d3197d877219ae513dc4b5` 和 QuickJS 2026-06-04。QuickJS 源码与 release commit `3d5e064e9dd67c70f7962836505a7fa067bf0a4e` 的 quickjs.c / quickjs.h blob 已核对相同。',
'同机 Ryzen 7 7840HS，CPU 2，五轮轮换顺序；未锁频，governor=powersave，存在桌面后台。计时包含整个进程；构建不与计时重叠。Tachyon 使用自建最小 host（上游 CLI 未实现），保留工作负载原体与 strict directive，单独加载 print/console.log，并提供 wall clock。正式矩阵统一限制 8 GiB 地址空间。Tachyon host 的 2 GiB managed heap、16,384 frames 等资源上限是显式比较配置，并非其仓库原有 benchmark_config 默认值。',
'Tachyon 普通构建：Rust 1.95.0 / thin LTO / 1 codegen unit / debug=2 / panic=abort。Oxide 普通历史基线为 Rust 1.94.1；为了检查构建混杂，将同一份 Oxide 源码改用前述 Tachyon 构建设置另测：','',
'| 负载 | Tachyon ms | 普通 Oxide ms | 匹配设置 Oxide ms | 匹配 Oxide / Tachyon |','|---|---:|---:|---:|---:|']
for case,es in s['control'].items():
 t,o,m=[es[e]['median_ms'] for e in ['tachyon','oxide','oxide_matched']]
 lines.append(f'| {case} | {t:.2f} | {o:.2f} | {m:.2f} | {m/t:.2f}× |')
lines+=['',
'匹配设置后差距仍存在，但这不是将每一项差距归因到某个架构决定的证明。真正隔离了单一设计参数的是上面的 batch 对照。',
'聚焦 profile 的两项纯计算负载中，empty_loop 约 96.8%、int_arith 约 96.3% 的样本落在已内联热内核的执行循环里；func_call 则可见执行循环、push_call_frame 和完整 dispatch 的分工。普通 ELF 用 cycles:u、周期 1,000,003、DWARF 16 KiB、per-thread 4 MiB buffer；只作叶子归因，不假定父调用栈可完整重建。',
'硬件指令数（三轮中位数，十亿条）如下；其中 opcode 数与硬件指令数不是同一指标：','',
'| 负载 | Tachyon instructions | Oxide instructions | QuickJS instructions |','|---|---:|---:|---:|']
for c in ['empty_loop','int_arith','func_call','string_length']:
 es=s['counters'][c]
 lines.append('| '+c+' | '+' | '.join(f'{es[e]["instructions:u"]/1e9:.3f}' for e in ['tachyon','oxide','quickjs'])+' |')
lines += ['',
'未运行完整 Test262。定向探针发现了语义差异，保留在机器可读数据中；因此负载输出通过不能外推为全面语义等价。此处的建议基于设计机制、针对性行为实验和受控 batch 对照，不依赖“另一个引擎整体更好”的前提。','',
'<details>','<summary>附录：全部 58 项固定负载时间（ms，中位数）</summary>','',
'| 负载 | Tachyon | Oxide | QuickJS |','|---|---:|---:|---:|']
for c,es in s['timing'].items():lines.append('| '+c+' | '+' | '.join(f'{es[e]["median_ms"]:.2f}' for e in ['tachyon','oxide','quickjs'])+' |')
lines+=['','</details>','',
'## 复现与证据位置','',
'- 新增接入与实验工具：`scripts/benchmark/tachyon/README.md`。普通 host、focused profiler、构建对照、batch ablation 和架构探针均有源码。',
'- 报告副本：`docs/reports/tachyon-architecture.md`；全部固定计时范围、计数器、profile、探针结果与构建哈希：`docs/reports/tachyon-comparison.json`。',
'- 原始 stdout/stderr、perf.data、计数器 CSV、固定 workload manifest、构建日志和诊断 patch 保留在 `target/tachyon-comparison/`，未声称公开上传。',
'- 广泛 profile 在研究范围收敛后已停止；未完成部分不计入结论。正式 profile 仅使用 `architecture/` 的 8 项、每项 3 轮。没有继续排查 Tachyon 的慢项。',
'- batch 对照只在外部实验 checkout 临时更改一处常量；测量使用单独保存的 ELF。该源码与原普通 ELF 已恢复并核对哈希。未修改 Oxide 生产代码，也未发布或修改 Tachyon 上游。','']
report='\n'.join(lines)
(ROOT/'docs/reports/tachyon-architecture.md').write_text(report)
(OUT/'architecture-comment.md').write_text(report)
(ROOT/'docs/reports/tachyon-comparison.json').write_text(json.dumps(s,indent=2)+'\n')
print('architecture report characters:',len(report))
