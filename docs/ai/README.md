# 激突Kit 造阵文档

从用户原码或STATE指定父阵开始，按[研究流程](formation-tuning.md)完成原阵研究、候选实验和交付。新任务先明确范围，续跑保留用户选择与停止状态。公共工具与模板见[实验复用目录](../../tools/ai/recipes/README.md)。

## 从准备到交付

| 要解决的问题 | 权威文档 |
|---|---|
| 工作区、后端与查源路线 | [项目地图](01-project-map.md)、[HP规则](unit-hp-rules.md) |
| 如何理解单位与配合 | [基础机制](08-formation-fundamentals.md)、[单位百科](knowledge/units/README.md)、[战术知识](knowledge/tactics/README.md) |
| 阵码、坐标与顺序 | [阵码契约](02-formation-format.md) |
| 怎样设计对照、解释联合收益和扩圈 | [搜索设计](04-search-workflow.md)、[搜索经验](11-search-lessons.md) |
| 完整执行顺序、批次研究和微调 | [研究流程](formation-tuning.md) |
| 阵池、胜率、完整性与选拔 | [阵集政策](10-benchmark-usage.md) |
| 如何生成候选与检查已有规则 | [生成器](generator.md) |
| 如何统一稳定态去重、处理意外位移 | [重力与布局验收](gravity-audit.md) |
| 怎样运行命令及复用公共脚本 | [工具手册](03-tool-runbook.md)、[复用目录](../../tools/ai/recipes/README.md) |
| 画像、热图、贡献和伤害来源能证明什么 | [分析使用](knowledge/analysis-usage.md) |
| 字段、索引、详细重放与查询 | [数据契约](data-contract.md)、[详细模式](detailed-mode.md)、[能力地图](capabilities.md) |
| 保存什么、怎样交接和沉淀知识 | [实验契约](05-experiment-contract.md)、[STATE模板](templates/STATE.md) |
| 怎样用子代理分工、短交接并核算消耗 | [子代理协作](subagent-workflow.md) |

首发/持续周期与真实命中查[弹幕时序](13-volley-timing.md)，防区位置、墙承伤与支撑分离查[防守部署](knowledge/defensive-positioning.md)，来源映射与模块迁移查[批量实验](knowledge/batch-design-dedup-defense.md)。[任务模板](PROMPTS.md)只填写本轮目标与约束。

## 当前工作链

`formation_workflow generate → stable_dedup run → 本轮预期/规则批量验收 → evaluate_pool → verify_pool`

稳定态去重通用，预期条件和检查程序随本轮任务配置。原码位移未通过不直接淘汰，未收敛单列补查；实际评测纯码与证据身份独立冻结。每批完整核验后先研究，再决定依赖反馈的下一批。具体参数、成绩与原始证据保存在Git忽略的本地实验目录，公开文档维护当前机制、方法和接口。
