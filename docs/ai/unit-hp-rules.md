# 当前后端与单位HP规则

Lab 0.6.6内置Kit 1.9.1。当前Java、Rust plain/basic/detailed与补丁JNI均采用catalog-hp-v1；冻结证据仍须记录各后端的实际构建与哈希。

| 后端 | 规则标识 | 蓝加速器（ID30）HP / max_hp | 电梯壁（ID60）HP / max_hp |
|---|---|---:|---:|
| 当前Java、Rust plain/basic/detailed、内置JNI与参考JAR | catalog-hp-v1 | 100 / 100 | 75 / 75 |
| 未经补丁的旧JNI | legacy-149e365 | 150 / 150 | 35 / 35 |

蓝加速器贡献2、死亡扣回2；修理上限读取实际max_hp。旧规则成绩与缓存不能视为新规则证据。

## 当前身份与构建

- Rust来源与规则：[PROVENANCE.json](../../rust/vendor/PROVENANCE.json)，当前源码和测试输入哈希：[source-hashes.json](../../rust/vendor/source-hashes.json)。
- 分发JNI及参考JAR的SHA256：[kit-manifest.json](../../app/kit-manifest.json)。JNI仅修正两种墙体的构造HP/max_hp及引擎标识；只读诊断路径经过等长去标识处理。公开工程不包含JNI crate，重新编译Java只会带入资源目录已有的DLL，不会从源码重建JNI。
- 当前构造：[Rust Wall::new](../../rust/vendor/gkt-core/src/engine/wall.rs)、[Java Turbo](../../src/main/java/org/example/elements/wall/Turbo.java)、[Java Elevator](../../src/main/java/org/example/elements/wall/Elevator.java)。

JNI引擎标识为`gkt-core (catalog-hp-v1)`，使对局及强度缓存与旧构建分开。prepare_core核验源码清单后生成基础/详细副本；观察器标识为`vendored-rust-149e365-catalog-hp-v1`。标识不能替代二进制哈希。

按[工具手册](03-tool-runbook.md#源码构建与显式seed)构建观察器。显式native路径或程序旁DLL可能覆盖JAR资源，评测前须确认实际加载身份。切换后端或版本后核验结果并重测基线，不混合不同构建的缓存；固定阵池仅提供输入。

## 可复用规则检查

[Rust HP检查](../../rust/observer/tests/unit_hp_rules.rs)覆盖plain/basic/detailed的构造HP、max_hp与修理上限；[Java检查](../../tests/UnitHpRulesCheck.java)核对构造及蓝加速贡献、死亡扣回。

[JNI检查](../../tests/NativeHpRulesCheck.java)使用程序生成的合成输入，在无治疗条件下耗尽蓝加速100、电梯壁75，并保留木壁35、铁壁150、红加速15对照；这些输入仅核验规则，不是合法比赛阵或强度样本。[隔离核验入口](../../tests/verify_native_hp.py)核对分发DLL指纹，并在临时目录编译和运行Java/JNI检查：

```powershell
conda activate opencode
python tests/verify_native_hp.py --out ai-data/verification/native-hp.json
```

`--jar <JAR路径>`可另行检查源码构建；输出拒绝覆盖。`tools/ai/test.ps1`同时核验参考JAR和源码JAR，再用`tests/verify_integration.py`对200局逐一核对JNI的status/winnerHp/frames与plain/basic/detail，并核对观察器初始HP/maxHp及详细账本。通过范围以实际报告为准，不构成原Flash等价证明。

[计时隔离检查](../../rust/observer/tests/unit_timing_knowledge.rs)核对壁玉恢复、理想周期空窗和炮弩计数；只推进有效计数，不模拟支撑或真实命中，不能代替强度评测。
