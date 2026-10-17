# 可复用搜索与选阵流程入口

本页连接各环节，完整步骤维护在[研究执行流程](formation-tuning.md)，避免再维护第二套流程。新任务显式指定自己的原阵、base和rules；旧任务先读STATE与本地实验记录，保留用户选择和停止状态。

| 环节 | 应使用的文档 | 完成后得到什么 |
|---|---|---|
| 理解输入与机制 | [基础机制](08-formation-fundamentals.md)、[单位百科](knowledge/units/README.md)、[阵码契约](02-formation-format.md) | 槽位职责、支撑、时序与可执行规则；需要时补[战术知识](knowledge/tactics/README.md) |
| 研究原阵 | [开搜前原阵研究](formation-tuning.md#11-开搜前先充分研究原阵) | 原码三池核验证据、200分群和代表胜败局、BASELINE与首批假设 |
| 设计有对照批次 | [搜索设计](04-search-workflow.md)、[搜索经验](11-search-lessons.md)、[批量实验](knowledge/batch-design-dedup-defense.md) | 父阵、精确改动轴、预期收益/副作用、失败判据和预算 |
| 生成与验收 | [生成器](generator.md)、[重力审核](gravity-audit.md) | 规则合法、零位移审核通过的新纯码与源阵映射 |
| 分池评测与选拔 | [执行步骤](formation-tuning.md#评测与逐池核验)、[三池政策](10-benchmark-usage.md) | 常规造阵200→2000→10433；已选基底微调默认2000→10433，各阶段完整成绩及verify证据 |
| 研究本批再定方向 | [持续候选研究](formation-tuning.md#22-搜索中持续研究候选再决定下一步)、[分析使用](knowledge/analysis-usage.md) | 逐对手收益/损失、关键解释的核验和下一批选择理由 |
| 停止、续跑与交付 | [运行现场](formation-tuning.md#后台评测与续跑)、[实验契约](05-experiment-contract.md) | 当前STATE、未完成项、真实局数台账与本地归档与公开知识提交 |

当前可调用链是`formation_workflow generate/accept → gravity.py evaluate → verify_pool.py`。进度读progress/summary；没有`evaluate / status / gate / rank-pool`生成包装子命令，也没有`--training-only / --cache-only / --final-test`包装选项。正文使用“门禁”描述完整性要求，不将它当作现存CLI子命令。

原阵三池研究与新候选分阶段评测是不同义务。原阵完成后，不因换内部父阵或生成稳定态机械重跑三池；新纯码不能继承旧成绩。缓存、跨池核验及两侧索引按[数据契约](data-contract.md)和执行流程处理，不能凭名称或相同成绩合并身份。
