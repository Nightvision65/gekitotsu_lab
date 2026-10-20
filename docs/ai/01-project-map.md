# 项目与引擎地图

## 工作区与后端

| 路径/后端 | 作用与使用边界 |
|---|---|
| `激突Kit.exe`、`app/激突Kit.cfg`、`app/gekitotsu_java-1.9.1.jar` | GUI发行包，内含JNI但无研究`--ai`入口；只读核对用kit_lab桥接 |
| `src/`、`pom.xml`、`target/gekitotsu_lab-0.6.7.7.jar` | 含GUI及`--ai`的研究JAR；ai.py/evaluate_pool使用此接口，本轮冻结副本用于后续评测与诊断 |
| JAR内JNI | 常规快速评测及GUI共享分析；冻结实际JAR和DLL哈希 |
| Java `GameTask` | 部分GUI功能的回退后端，不能与JNI混缓存或直接拼成绩 |
| `rust/vendor/gkt-core/` | 当前派生Rust核心；来源和逐文件身份见[PROVENANCE](../../rust/vendor/PROVENANCE.json)及[source-hashes](../../rust/vendor/source-hashes.json) |
| `rust/observer/` | plain/basic/detail对照、显式seed及战场遥测；基础/详细程序分别记录构建身份 |
| `rust/gravity/` | 独立目标目录的静态支撑模型，不运行完整战斗 |
| `tools/ai/`、`tools/ai/recipes/` | 生成、稳定态去重、评测、存证、查询及实验复用工具 |
| 原Flash或实际比赛环境 | 验证可导入性、微妙碰撞和目标环境表现 |

当前HP为[catalog-hp-v1](unit-hp-rules.md)。JNI缺少公开构建工程，Java打包携带资源目录中已有DLL；重建Java不等于重建JNI。独立Rust可编译阵码、运行单局/批量、显式种子和观察器，不自带HTTP服务、Python包或自动造阵调度。

GUI与CLI能力对应见[能力地图](capabilities.md)，环境与调用见[工具手册](03-tool-runbook.md)。

## 引擎核验与证据范围

证据身份包括纯码、对手、物理侧位、帧上限、seed政策、二进制及适配器哈希。JNI实际加载DLL的SHA256必须单独保存，引擎显示名不能代替构建身份。Java和Rust的随机更新路径存在差异，不因一次胜负一致就混用；换后端或构建后核验并补齐对应基线。

同输入逐局比较status、winnerHp和frames；plain/basic/detail还可比较双方核心HP与最终RNG。详细诊断再核对HP账本、来源归因、事件完整性及与基础结果的一致，缺原始raw时不能把摘要当已复验。保存probe、请求、构建指纹和原始文件哈希，复用前检查。

[公开测试](../../tests/)及[Rust观察器测试](../../rust/observer/tests/)按当前构建运行。缺少telemetry/frames金样而跳过时不计作对拍通过；源码公式核对不等于GUI视觉验收，模拟器一致也不证明原Flash等价。失败、缺局和进程超时不补成平局。性能结论只对机器、输入及计时范围有效，详细遥测按具体问题开启。

## 查源码的路线

| 问题 | Java（相对`src/main/java/`） | Rust（相对`rust/vendor/gkt-core/`） |
|---|---|---|
| 阵码、分隔与读取 | `org/example/Setting.java`、`Main.java`、`GUI/Unit.java`、`GUI/Formation.java` | `src/decode.rs` |
| 结果与帧循环 | `org/example/GameTask.java`、`Result.java` | `src/engine/mod.rs` |
| 单位与构造属性 | `org/example/GUI/Unit.java`、`elements/units/` | `src/engine/ball.rs` |
| 加速、突击、转墙 | `elements/wall/`、`GUI/Trace.java`、`GUI/AssaultDetector.java` | `src/engine/wall.rs`、`base.rs` |
| 贡献、画像、战场聚合 | `ContributionAnalyzer.java`、`MatchupAnalyzer.java`、`BattlefieldAnalyzer.java` | 详细记录见`rust/observer/src/` |
| GUI效果 | `org/example/GUI/effects/` | 无对应GUI效果层 |

先按实际ID读单位百科；缺项、矛盾、版本变化或新问题超出条目时再追源码，并将确认机制补回对应主题。
