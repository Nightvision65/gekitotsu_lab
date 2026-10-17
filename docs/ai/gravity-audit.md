# 批量开局重力审核

政策：`static-support-gravity-v1`。适用于用户要求的“开局很快自然变成的布置”。坐标为`kit-workbench`，slot0是核心，保留原单位顺序、类型、x和角度。

只运行常规重力与静态支撑：直接调用现存Rust核心的`Ball::land`，包含下落、被墙体或核心形状挤起、车板承托和实际坐标量子化。普通兵玉不互相承托。核心先放到实际世界位置再参与碰撞，不能拿构造时的车体相对坐标做支撑判定。

**全部特殊效果禁用**：车体不推进、不碰撞；升降壁、远突壁、旋转壁、狙击壁等全部固定初始位置，仅保留实体形状；不执行定时触发、突击、狙击转态、旋转、攻击、治疗、召唤或飞行能力。输出是这个统一静态模型的稳定几何，不是模拟完整对局若干帧后的截图，也不保证完整战斗中之后不再移动。

## 使用

所有Python命令先激活`opencode`。首次构建需现存研究版的`rust/vendor/gkt-core`源码和Rust工具链；依赖使用本地缓存，默认离线构建。

```powershell
conda activate opencode
python tools/ai/gravity.py build
python tools/ai/gravity.py audit --input candidates.json --out experiments/my-run/gravity-audit
python tools/ai/gravity.py apply --input candidates.json --out experiments/my-run/gravity-applied
```

输入支持候选JSON数组、含`candidates`的JSON对象，或Kit的`名称&代码/名称&代码`文件；多阵必须用`/`分隔。不修改源文件。输出目录必须是新目录，禁止覆盖历史审核。

`audit`只审核原码，退出码0代表全部通过，3代表至少一阵不通过，但完整报告仍会保存。每阵输出逐slot初始/稳定坐标、位移、最大上移/下落、最后移动帧和支撑对象。`auditPassed`要求完整收敛且全部单位零位移，浮点比较容差0.001px，不放过1px变化。

`apply`输出稳定态新码，并再次运行重力审核；只有全部新码零位移且回编码幂等时才发布`candidates.json`和`formations.txt`。多个源阵收敛成同一码时，保留首次顺序与名称，`aliases.json`保存源索引/别名映射。旧分数和旧码哈希不会复制到新候选。无法收敛、坐标越界或不能整数编码时整批失败，不截断、夹紧、删除单位或偷偷发布部分成功阵。

默认最多240帧，连续12帧位置不变且支撑落体速度为零才判稳定；已到地面则检查其位置不变。两项可用`--max-frames`和`--stable-frames`调整，但必须`2 <= stable-frames <= max-frames <= 2000`。底层进程还有真实超时，超时不算稳定。

## 造阵评测的强制入口

新增或修改候选一律先过审核。

开搜前用户要求的未改动来源原码三池基线，按[原阵研究](formation-tuning.md#11-开搜前先充分研究原阵)处理：先保存审计；若原码不稳定，只读诊断可直接使用`evaluate_pool.py`，明确不具备新候选晋级资格。不得用此边界放行任何新建、修改或apply后的候选，稳定态不能继承来源原码成绩。

新候选建议直接使用带门禁的评测入口，失败时不会启动任何对局：

```powershell
python tools/ai/gravity.py evaluate --candidates candidates.json --audit-out experiments/my-run/preflight --evaluation-timeout 1800 -- --opponents benchmarks/standard200/opponents.txt --side 1 --store experiments/my-run/store.sqlite --out experiments/my-run/evaluation --threads 32 --timeout 300 --time-limit 1700
```

`--`前是重力参数，后是现有`evaluate_pool.py`参数；候选只在前面指定一次。2000与10433同样使用该入口。未通过时先修正坐标，或对`apply`的输出重新评测，不能静默替换输入。重复评测可指定已有的同一审核目录：它先核验输入、工具版本、结果哈希和零位移门禁，再复用审核。

独立复核审核证据：

```powershell
python tools/ai/gravity.py verify --run experiments/my-run/gravity-applied --input candidates.json
```

这里`--input`始终是该次审核/应用的**源输入**，应用输出的哈希另行核对。源码、二进制、解析器或输入改变后，必须在新目录重新审核。正常对局的证据缓存仍按原对战引擎身份核验，不能因为重力模型相同就复用不同纯码的成绩。

冻结对手池不应用此转换、不删改权重；可只读诊断。重力审核不替代军资、禁限、边界、核心、单位顺序与火力时序审计。

## 实现与验证

实现为独立构建的`rust/gravity`和`tools/ai/gravity.py`，不会覆盖原JAR、DLL、观察器或GUI输入。GUI旧`FormationStabilizer`只是第一帧挤压近似，不能冒充本功能的连续重力审核。

静态模型可通过[Rust机制测试](../../rust/gravity/src/main.rs)、[Python门禁测试](../../tests/test_gravity_gate.py)及[Java重力oracle](../../tests/GravityOracle.java)核验。跨实现检查时冻结相同输入、侧位和迭代范围，逐槽比较稳定坐标；这种核对支持静态模型一致性，不代表Java与Rust完整战斗后端可混用。

## 侧位与条件功能的补充核验

当前rust/gravity/src/main.rs虽然创建双方，但实际推进并导出的ids只含side=0；通用审核通过只证明该静态模型下的1P，不可自动声称2P也通过。单位生成坐标有明确镜像公式，并不存在可统一套用的“2P工作台x加1”；墙形状边缘与更新顺序仍可能产生侧差。对卡边、掉落等敏感功能，应实际实例化两侧核验。

墙损坏触发掉落时，按“带支撑稳定→删除实际支撑对象→继续稳定”记录前后位置与支撑种类，两侧分别检查。交叉核验应保存输入、构建哈希与前后位置，只解释声明的核心类型和静态实体模型，不启动攻击，不替代完整战斗胜率；人为1px偏移仅是敏感性控制，不能称为默认2P偏移。

