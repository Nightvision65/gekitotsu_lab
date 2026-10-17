# 当前后端与单位HP规则

Lab 0.6.5内置Kit 1.9.1。源码观察器与内置JNI是两条独立的评测线，按实际使用的后端冻结规则和哈希。

| 后端 | 规则标识 | 蓝加速器（ID30）HP / max_hp | 电梯壁（ID60）HP / max_hp |
|---|---|---:|---:|
| Java源码、Rust plain/basic/detailed | catalog-hp-v1 | 100 / 100 | 75 / 75 |
| 内置JNI、固定参考JAR | legacy-149e365 | 150 / 150 | 35 / 35 |

工作台目录值为100/75；目录展示不决定内置JNI的战斗构造值。蓝加速器贡献2、死亡扣回2。修理上限读取实际后端的max_hp。

## 当前身份与构建

- Rust来源与规则：[PROVENANCE.json](../../rust/vendor/PROVENANCE.json)，当前源码与测试输入哈希：[source-hashes.json](../../rust/vendor/source-hashes.json)。
- JNI及参考JAR：[kit-manifest.json](../../app/kit-manifest.json)。公开工程不包含JNI crate，重新编译Java不会重建嵌入的DLL。
- 当前构造：[Rust Wall::new](../../rust/vendor/gkt-core/src/engine/wall.rs)、[Java Turbo](../../src/main/java/org/example/elements/wall/Turbo.java)、[Java Elevator](../../src/main/java/org/example/elements/wall/Elevator.java)。

prepare_core核验当前源码清单，再生成基础/详细副本；generated-hashes记录ruleset。观察器输出engine为vendored-rust-149e365-catalog-hp-v1，二进制SHA256继续参与缓存身份。

按[工具手册](03-tool-runbook.md#源码构建与显式seed)构建观察器，为ai.py显式指定对应的`--observer`与`--detailed-observer`并使用独立store。切换后端后重测基线，不混合不同规则的成绩与缓存；固定阵池仅提供输入。

## 可复用规则检查

[Rust HP检查](../../rust/observer/tests/unit_hp_rules.rs)覆盖plain/basic/detailed，核对ID30/60的生成HP、max_hp和repair上限；[Java检查](../../tests/UnitHpRulesCheck.java)核对构造值与蓝加速贡献及死亡扣回。

`tests/verify_integration.py`核对catalog-hp-v1的plain/basic/detail结果；JNI接口单独检查。`--compare-jni`执行严格逐局对照，规则不一致会失败。默认验收不构成跨后端等价证明。

[计时隔离检查](../../rust/observer/tests/unit_timing_knowledge.rs)核对壁玉恢复、理想周期空窗和炮弩计数；只推进有效计数，不模拟支撑或真实命中，不能代替强度评测。
