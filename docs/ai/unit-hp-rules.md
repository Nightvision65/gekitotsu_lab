# 单位HP规则：catalog-hp-v1

当前Java、派生Rust核心和发行包JNI采用以下构造HP与maxHp；修缮上限使用同一maxHp，目录显示值不能代替实际构造检查。

| 单位 | ID | HP / maxHp |
|---|---:|---:|
| 蓝加速器 | 30 | 100 |
| 电梯壁 | 60 | 75 |
| 木要塞壁 | 25 | 35 |
| 铁要塞壁 | 28 | 150 |
| 红加速器 | 29 | 15 |

蓝加速贡献2级，死亡扣回2级。当前Rust构造见[Wall::new](../../rust/vendor/gkt-core/src/engine/wall.rs)，Java见[Turbo](../../src/main/java/org/example/elements/wall/Turbo.java)与[Elevator](../../src/main/java/org/example/elements/wall/Elevator.java)。来源和逐文件身份由[PROVENANCE](../../rust/vendor/PROVENANCE.json)、[source-hashes](../../rust/vendor/source-hashes.json)记录；生成基础/详细核心副本前须核验源码清单。

## 运行构建身份

分发DLL与参考JAR的SHA256由[kit-manifest.json](../../app/kit-manifest.json)记录，JNI标识为`gkt-core (catalog-hp-v1)`。研究观察器标识规则版本，但缓存身份仍用实际二进制/适配器哈希。

JNI缺少公开构建工程，Java打包携带已有资源DLL。显式native路径或程序旁DLL可覆盖JAR资源，评测前应以probe确认实际加载构建，不能只看文件名或标识字符串。不同HP规则或后端分库，重新核验对应基线后比较；阵池输入和来源排名本身不证明当前构建下的战果。

## 规则验证

- [Rust HP检查](../../rust/observer/tests/unit_hp_rules.rs)：plain/basic/detail的实际生成HP、maxHp和repair上限，含木壁、红加速对照。
- [Java检查](../../tests/UnitHpRulesCheck.java)：构造HP及蓝加速贡献、死亡扣回。
- [引擎集成检查](../../tests/verify_integration.py)：当前HP/maxHp及JNI、plain/basic/detail逐局结果对照。
- [计时隔离检查](../../rust/observer/tests/unit_timing_knowledge.rs)：真实初始化计数、壁玉恢复与理想周期；不模拟实际支撑或命中。

构建与运行命令见[工具手册](03-tool-runbook.md#源码构建与显式seed)。规则检查通过不代替阵型强度评测，也不证明原Flash等价；精确后端对照要求见[引擎证据范围](01-project-map.md#引擎核验与证据范围)。

## 分发包的实际构造核验

[JNI隔离检查](../../tests/verify_native_hp.py)依据分发清单核对指纹，并使用公开的[合成Java检查](../../tests/NativeHpRulesCheck.java)验证实际Java/JNI构造与耗尽HP。`tools/ai/test.ps1`同时检查固定参考Kit JAR与本版研究JAR，再执行完整200局跨后端严格对照。合成输入不作为合法比赛阵或胜率样本。
