# 能力地图与实现决策

本页按研究问题选择实际接口；运行方法见[工具手册](03-tool-runbook.md)，字段与查询约束见[数据契约](data-contract.md)。能力描述不构成修改战斗核心的授权。

## GUI信息如何交给AI

| 研究问题 | AI入口/查询表 | 数据来源及限制 |
|---|---|---|
| 单位图鉴、说明、战术与属性 | `catalog` → `units`；`--fields id,name,detail,catalogHp,cd,at,shoot` | 原`Unit.infos`、`unit_details_zh.json`；目录HP、伤害编码不能冒充实际战斗属性 |
| 工作台解码、成本、加速等级、顺序 | `formation` → `formations/units` | 原`Formation/Unit`；输入严格验证，拒绝静默清洗或裁剪；只读，不输出修改后的阵 |
| 重叠/突击最速/壁玉/枪玉/愈缮顺序建议 | `formation` → `issues/assaultGroups` | 新共享`WorkbenchAnalysis`，GUI也调用；经验建议不等于胜率结论 |
| 卡墙挤压后位置 | `formation` → `units.stabilizedX/Y` | 原`FormationStabilizer`，是1P初始几何辅助模型 |
| 突击轨迹/要塞运动公式 | `run`，action=`trajectory` → `trajectory` | 新共享`TrajectoryModel`，原Canvas绘制同样调用；分段公式假设固定加速，实际加速器死亡等变化要查battle |
| 批量对战与换边 | `evaluate` → `forward/reverse/scores` | 原Rust JNI，无Java回退；完整N作分母，U计平局；双侧结果分别保存 |
| 单删/双删贡献 | `analyze --kind contribution` → `combos/baselineResults` | 使用原DLL重测；穷举、成本/原槽位明确；直接保留原六字符块；不使用GUI随机采样 |
| 对局画像、单位与加速相性 | `analyze --kind matchup` → `sensitivity/acceleration/opponents` | 复用原GUI分析与原始逐局结果，另按新口径计算完整子集成绩；相关性不是因果 |
| Keener辅助强度 | `analyze --kind matchup --strength` → `strengthEntries/strengthSummary` | GUI实现；默认不开，先检查maxJobs；辅助算法与主胜率分列 |
| 战场空间、受创、死亡、推进分箱 | `analyze --kind battlefield` → `slots/gameScalars/timeline` | 原JNI聚合；映射阵码核心slot0，均值保留sampleCount；rawGuiReport仍完整留存 |
| 原GUI没有开放的逐单位关键事件 | `battle/diagnose` → `units/events/timeline` | 可重建的公开Rust核心＋新增只读观察器，详情见数据契约 |
| 指定对局详细研究 | `replay`或`battle --detailed` → `combatEvents/objects/unitStats/damageSources/damageRootSources/healingSources/flightSamples` | 精确HP写点、攻击和实际生成链，含核心与墙；冻结基础请求，核对结果；同帧重叠按实际首个命中见证记账 |
| 多局生命周期与突击落点 | `cohort --manifest` | 按槽位、地面/己方平台/敌方平台分别统计位置；不预设登阵目标；死亡/半血均值保留删失边界 |
| 父阵与候选的关键变化 | `compare` | 同引擎、同对手、同seed、同帧上限；对应槽位必须人工确认，不能自动跨调序映射 |

## 为什么采用SQLite与CLI

实验外围自动化集中在[公共脚本与模板](../../tools/ai/recipes/README.md)：建档与输入快照、命令/PID/日志/真实超时、统一稳定态去重、已核验完整池排名/分群/真实父阵增失分、文件与SQLite引用raw核验及静态脚本盘点。本轮预期使用任务检查程序和模板，不自动续搜。

完整JSON作为不可覆盖证据；SQLite保存事件、单位、时间线与分析记录的索引；stdout只给摘要或有界分页。AI可以直接调用Python命令，不需要开启服务。缓存、查询、比较、校验共享同一数据契约，MCP并不是节省上下文的必要条件。

默认一页20行、最大100行、默认12000字节；可以按side、slot、entityId、kind、frame、formationIndex筛选，按字段投影，按offset分页。个别巨行超预算会明确返回提示，不截断成半个JSON。

大规模候选仍只跑结果；稀疏遥测用于少量有解释价值的对局。每局检查结果，不把程序超时当作模拟器帧上限平局。只有确实需要更多细节时才查询原始分析行。

## 后端与观察范围

Java通过资源中的`gkt_jni.dll`评测；公开Rust无JNI crate。gkt-observe/gkt-detail为独立程序，通过请求/输出JSON连接调用器。`prepare_core.py`验证冻结vendor后生成basic-core/diagnostic-core副本：基础版插入满血恢复信号，详细版增加HP写点、来源链与移动记录，保留物理、伤害、RNG及插入顺序。plain链接同一经哈希核验的vendor、不加观察钩子；当前构建采用[catalog-hp-v1](unit-hp-rules.md)。

JNI、基础与详细观察器各自冻结指纹，详细程序不链接进基础程序。有限输入对照只证明该输入与构建范围，不能宣称所有未来构建或原Flash等价。记录点按精确上下文验证，生成文件不入库；新克隆先运行build.ps1或prepare_core.py。

`rust/vendor/PROVENANCE.json`记录来源，`source-hashes.json`逐文件验证当前源码及公开fixtures，Cargo.lock冻结依赖。未分发的telemetry/frames金样不作为本地可执行前提；验收明确缺失或跳过范围，核验边界见[项目地图](01-project-map.md#引擎核验与证据范围)。
