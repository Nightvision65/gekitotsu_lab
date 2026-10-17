# 能力地图与实现决策

研究版提供GUI共享分析、基础/详细观察器和本地存证查询。能力描述不构成后续任务修改源码的授权。上游Java固定入口提交 `bc90ce0e5e624ff1594603ae46a4b44159414df4`；公开Rust固定提交 `149e365221b09330a72d9a6a1804605e3ce4e1f5`。发行包和历史实验保留在原目录。

## GUI信息如何交给AI

| 原GUI功能 | AI入口/查询表 | 数据来源及限制 |
|---|---|---|
| 单位图鉴、说明、战术与属性 | `catalog` → `units`；`--fields id,name,detail,catalogHp,cd,at,shoot` | 原`Unit.infos`、`unit_details_zh.json`；目录HP、伤害编码不能冒充实际战斗属性 |
| 工作台解码、成本、加速等级、顺序 | `formation` → `formations/units` | 原`Formation/Unit`；输入严格验证，拒绝静默清洗或裁剪；只读，不输出修改后的阵 |
| 重叠/突击最速/壁玉/枪玉/愈缮顺序建议 | `formation` → `issues/assaultGroups` | 新共享`WorkbenchAnalysis`，GUI也调用；经验建议不等于胜率结论 |
| 卡墙挤压后位置 | `formation` → `units.stabilizedX/Y` | 原`FormationStabilizer`，是1P初始几何辅助模型 |
| 突击轨迹/要塞运动公式 | `run`，action=`trajectory` → `trajectory` | 新共享`TrajectoryModel`，原Canvas绘制同样调用；分段公式假设固定加速，实际加速器死亡等变化要查battle |
| 批量对战与换边 | `evaluate` → `forward/reverse/scores` | 原Rust JNI，无Java回退；完整N作分母，U计平局；双侧结果分别保存 |
| 单删/双删贡献 | `analyze --kind contribution` → `combos/baselineResults` | 使用原DLL重测；穷举、成本/原槽位明确；直接保留原六字符块；不使用GUI随机采样 |
| 对局画像、单位与加速相性 | `analyze --kind matchup` → `sensitivity/acceleration/opponents` | 复用原GUI分析与原始逐局结果，另按新口径计算完整子集成绩；相关性不是因果 |
| Keener辅助强度 | `analyze --kind matchup --strength` → `strengthEntries/strengthSummary` | 原GUI实现；默认不开，先检查maxJobs；历史口径明确标注 |
| 战场空间、受创、死亡、推进分箱 | `analyze --kind battlefield` → `slots/gameScalars/timeline` | 原JNI聚合；映射阵码核心slot0，均值保留sampleCount；rawGuiReport仍完整留存 |
| 原GUI没有开放的逐单位关键事件 | `battle/diagnose` → `units/events/timeline` | 可重建的公开Rust核心＋新增只读观察器，详情见数据契约 |
| 指定对局详细研究 | `replay`或`battle --detailed` → `combatEvents/objects/unitStats/damageSources/damageRootSources/healingSources/flightSamples` | 精确HP写点、攻击和实际生成链，含核心与墙；冻结基础请求，核对结果；同帧重叠按实际首个命中见证记账 |
| 多局生命周期与突击落点 | `cohort --manifest` | 按槽位、地面/己方平台/敌方平台分别统计位置；不预设登阵目标；死亡/半血均值保留删失边界 |
| 父阵与候选的关键变化 | `compare` | 同引擎、同对手、同seed、同帧上限；对应槽位必须人工确认，不能自动跨调序映射 |

## 为什么采用SQLite与CLI

完整JSON作为不可覆盖证据；SQLite保存事件、单位、时间线与分析记录的索引；stdout只给摘要或有界分页。AI可以直接调用Python命令，不需要开启服务。缓存、查询、比较、校验共享同一数据契约，MCP并不是节省上下文的必要条件。

默认一页20行、最大100行、默认12000字节；可以按side、slot、entityId、kind、frame、formationIndex筛选，按字段投影，按offset分页。个别巨行超预算会明确返回提示，不截断成半个JSON。

大规模候选仍只跑结果；稀疏遥测用于少量有解释价值的对局。每局检查结果，不把程序超时当作模拟器帧上限平局。只有确实需要更多细节时才查询原始分析行。

## Rust接口缺口的处理

原Java仓库只有构建好的`gkt_jni.dll`，公开Rust仓库没有JNI crate。因此采用两个独立可执行程序gkt-observe/gkt-detail，通过请求/输出JSON与AI调用器连接。`prepare_core.py`验证冻结vendor后生成basic-core/diagnostic-core副本；基础副本只插入满血恢复信号，详细副本增加HP写点、对象来源链和移动阶段记录点。vendor不回写，物理、伤害、RNG及插入顺序保留。plain链接当前经哈希核验的vendor core，不加观察钩子；该vendor含catalog-hp-v1本地修正，不能称未经修改的上游。

原DLL继续承担快速评测和GUI分析；基础与详细观察器各自记录二进制SHA256，详细程序不链接进基础程序。当前200局对照通过只是本次冻结输入上的证据，不能自动宣称任何未来构建、原Flash或整个引擎都等价。生成器按精确上下文核验每个记录点，生成文件不入库；新克隆先运行build.ps1或prepare_core.py。

`rust/vendor/PROVENANCE.json`记录来源，`source-hashes.json`逐文件验证当前规则源码和公开fixtures；当前核心含[目录HP修正catalog-hp-v1](unit-hp-rules.md)。Cargo.lock冻结新增依赖；原公开测试材料一起保留。未公开的telemetry/frames金样仍缺失，验收要报告跳过项。
