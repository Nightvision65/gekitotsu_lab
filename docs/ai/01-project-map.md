# 项目与引擎地图

## 当前工作区

本地同时包含 Windows 发行包、Java研究版源码（`src/`、`pom.xml`）和Rust观察/重力工具（`rust/`）。公开源码构建`gekitotsu_lab`，发布启动器为`gekitotsu_lab.exe`，主类为`org.example.Main`。`app/gekitotsu_java-1.9.1.jar`仅保留为旧Kit桥接的固定参考二进制；运行时与启动器由发布构建生成。该 JAR 内含 `native/gkt_jni.dll`，实测可加载。

| 层 | 主要作用 | 后续造阵的用法 |
|---|---|---|
| Kit GUI | 对战、工作台、轨迹、图鉴、行为分析 | 预览结构，检查单位与落点，解释失败 |
| Kit 公开 Java 接口 | `Formation`、`RustBattle`、各 Analyzer | AI 从外部调用，输出结构化结果 |
| Kit 内置 Rust DLL | 单局、批量、战场遥测 | 默认评测后端，无需重新编译 Rust |
| Kit Java `GameTask` | Rust 不可用时部分功能的回退 | 单独验证或排查；与 Rust 结果分库 |
| 独立 `gekitotsu_rust` | `gkt-core` 纯 Rust 核心库 | 显式种子、原始事件/快照、批量搜索接入 |
| 原 Flash 游戏或目标比赛环境 | 实际规则与表现 | 验证可导入性、微妙碰撞、关键胜负 |

## Kit 的实际功能

1. **对战模拟**：`1P × 2P` 笛卡尔积，导出详细和简要结果；不会自动反向换边。
2. **行为分析 → 贡献分析**：删除非核心槽位中的单位组合，比较删前删后得分；墙和加速器也在枚举范围内。
3. **行为分析 → 对局画像**：对手单位存在/不存在时的相性、加速等级分组、核心位置和可选 Keener 强度修正。
4. **行为分析 → 战场分析**：按胜负筛选对局，聚合槽位受创/阵亡、推进时间线及核心首次受击等信息；要求 Rust，无法以 Java 完整回退。
5. **阵型工作台**：解码、预览、单位序列、成本与加速等级、效果工作流，以及重叠、突击最速、壁玉位置等提示。
6. **轨迹预测与单位图鉴**：帮助构造突击落点、行动时序和单位搭配假设。轨迹计算是辅助模型，不能代替完整对战。

功能名与布局以1.9.1源码中的`MainGUI`、`BehaviorAnalysisTab`和本地类接口为依据。贡献分析属于行为分析子页；GUI具体操作见[使用手册](../../使用手册.md)。

## 独立 Rust 仓库能补什么

固定提交为 `149e365221b09330a72d9a6a1804605e3ce4e1f5`，随仓库分发的源码位于`rust/vendor/gkt-core/`；测试输入位于`rust/fixtures/`。

| 接口 | 已有能力 | 注意事项 |
|---|---|---|
| `decode::compile_fort(name, code)` | 编译干净阵码 | 不负责比赛合法性；不能直接喂 `name&code#HP` |
| `GameTask::run_single(p1, p2, max_frames, seed)` | 单局，`None` 或 `Some(i32)` 种子 | 返回任务终态与结果 |
| `run_batch(&jobs, threads)` | 并行独立对局 | 返回顺序与 jobs 一致；`BatchJob` 中保留双方引用 |
| `BatchOutcome` | 结果、终局 RNG、双方 HP | 比 Kit 的四字段结果更适合一致性检查 |
| `BattleRecorder` / `run_single_observed` | 事件、快照与汇总 | 大量候选初筛时关闭遥测，只对入围者开启 |
| `run_batch_with` | 每局独立 observer 的批量执行 | 不等于自动跨进程或 Python 接口 |

公开 Cargo workspace 仅包含 `crates/core`、`tools/gktdiff`、`tools/gktbench`。**没有 `gkt-jni`、HTTP 服务、Python 包或完整造阵优化器。** 内置JNI身份见[Kit清单](../../app/kit-manifest.json)，公开源码不能独立重建此DLL。

不同后端的一致性须按当前构建和公开输入实际核验。

## 为什么要锁引擎

本地 Java `Utils` 的初始种子为 0，并使用 double 运算的旧随机更新路径；独立 Rust 固定 oracle 的默认种子为 1，并采用 long LCG 语义。这些随机更新路径不同，不能只凭一次胜负一致就视为相同引擎；比较时须同时检查终态HP与结束帧。

固定内置 DLL + 相同阵码 + 同一侧位 + 同一帧数，作为一条评测线。更换后端或升级 JAR/DLL 后，重新跑原基线，不拼接之前的排行榜。引擎显示字符串 `gkt-core (bit-exact rust)` 不包含精确构建版本，因此还必须保存 DLL 的 SHA256。

细微卡位、多段突击与精准命中等结构仍须在原游戏核验。

## 引擎核验与证据范围

普通评测使用冻结JAR内的JNI DLL，基础/详细诊断使用独立观察器；plain是不加观察钩子的当前vendor核心。三者分别记录二进制、源码/适配器和输入哈希，不能用相同引擎显示名、源码提交号或一份测试摘要推断当前构建完全相同。当前派生源码的HP规则见[版本说明](unit-hp-rules.md)，重建源码不等于重建缺少公开工程的JNI。

对照时按同纯码、对手、物理侧位、帧上限和seed逐局比较。JNI可观察的status、winnerHp和frames一致，只支持这些字段的一致；原生plain/basic/detail还可核对双方核心HP与最终RNG。详细模式再检查HP账本、来源金额、事件完整性及与基础模式的结果一致，不能把分类修复、统计完整和全量事件完整混为一谈。执行与复现见[工具手册](03-tool-runbook.md#源码构建与显式seed)。

在本地保存probe、逐局对照、规则核验及其冻结请求、构建指纹和原始文件哈希。复用前检查这些文件；缺失完整raw时不把摘要当作本地已复验，旧HP版本的成功也不适用于新规则。可分发的规则检查程序位于[tests](../../tests/)与[Rust观察器测试](../../rust/observer/tests/)，由使用者针对当前构建运行。

未公开的telemetry/frames大金样会导致相关套件跳过，返回成功不表示金样对拍通过。GUI文本/轨迹公式核对不等于逐控件视觉验收，模拟器对照也不证明原Flash等价。构建或运行失败不生成成功证据，更不能补成对局平局。性能数字只对其机器、输入和计时范围有效；常规造阵用无遥测批评测，详细模式按问题开启。

## 查源码的路线

| 问题 | Java 源码位置（相对 `src/main/java/`） | Rust 源码位置（相对 `rust/vendor/gkt-core/`） |
|---|---|---|
| 编码、分隔、文件读取 | `org/example/Setting.java`、`Main.java`、`GUI/Unit.java`、`GUI/Formation.java` | `src/decode.rs` |
| 结果与帧循环 | `org/example/GameTask.java`、`Result.java` | `src/engine/mod.rs` |
| 单位与成本 | `org/example/GUI/Unit.java`、`elements/units/` | `src/engine/ball.rs` |
| 加速、突击、转墙 | `elements/wall/`、`GUI/Trace.java`、`GUI/AssaultDetector.java` | `src/engine/wall.rs`、`base.rs` |
| 贡献与相性 | `ContributionAnalyzer.java`、`MatchupAnalyzer.java`（均在 `org/example/`） | 不提供同等 GUI 分析器 |
| 战场数据 | `org/example/BattlefieldAnalyzer.java` | `src/engine/telemetry.rs` |
| 内置效果 | `org/example/GUI/effects/` | 不提供 |

微调前按解码ID查阅[单位百科](knowledge/units/README.md)，覆盖本阵核心、兵玉、保护墙和加速器，理解职责、移动/支撑、攻击、受击与生成对象。对方值得研究的单位也可随时查阅。仅在百科缺项、机制矛盾、版本变化或新问题超出条目时追源码，并把确认结论补回相应页面；无需每轮重读所有单位源码。当前源码含[HP修正规则](unit-hp-rules.md)，与发行包旧DLL分开冻结。
