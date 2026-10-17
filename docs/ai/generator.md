# 通用参数生成器

当前入口为`tools/ai/formation_workflow.ps1 generate / accept`，用于短JSON计划生成与稳定态规则验收，不负责自动搜索调度、选拔或阵池重排。评测与核验由[研究执行流程](formation-tuning.md)衔接；当前没有其他生成包装子命令。

## 计划与规则

执行前阅读[基础机制](08-formation-fundamentals.md)、[阵码契约](02-formation-format.md)及本阵单位百科。每份计划显式填写`base`、`rules`、`maxCandidates`、`groups`。`base`和字符串形式的`rules`路径相对工作区根目录；`--plan / --out`按调用时路径解析。JSON、阵码文件均为UTF-8，多阵必须用`/`分隔。

`base`支持候选JSON数组、含`candidates`的对象及Kit阵码文件，必须包含`rules.parentCode`。每个group默认使用base中的所有父阵，也可用`parents`显式列出父阵对象；所有父阵均须满足本轮冻结规则。规则中的固定字段与配对以`rules.parentCode`为参照，候选变化与偏移则以本次具体父阵为参照。

```json
{
  "base": "experiments/next/parent.txt",
  "maxCandidates": 8,
  "rules": {
    "profile": "coordinate-micro-v1",
    "parentCode": "<替换为本轮父阵纯码>",
    "budget": 1500,
    "fixed": [{"slot": 0, "fields": ["id", "x", "y", "r"]}],
    "pairs": []
  },
  "groups": [{
    "name": "local-angle",
    "hypothesis": "只改变目标单位角度，比较相邻方向",
    "axes": [{"slots": [1], "field": "r", "offsets": [-2, -1, 1, 2]}]
  }]
}
```

示意含占位符，不能直接运行。slot须按实际解码替换；不套用任何案例的固定槽位。

规则支持：

| 字段 | 含义 |
|---|---|
| `profile` | 必填。`coordinate-micro-v1`保留类型、数量和槽位顺序；`structural-fixed-speed-v1`允许非核心换兵及完整template，并要求显式`accelLevel` |
| `parentCode / budget` | 必填。冻结参考父阵与正整数军资上限；每个候选重新计算预算 |
| `coreType / coordinates` | 当前仅允许普通核心0与`kit-workbench`，省略时也只取这两个值 |
| `fixed` | `{"slot":3,"x":54}`固定显式值；`{"slot":0,"fields":["x","y"]}`保持父阵值，两种写法不混用 |
| `pairs` | `{"slots":[1,2],"fields":["x","y"]}`保持有向相对差；也可指定`dx / dy / signedXGap / sameY / unitId`，差值方向为第二槽减第一槽 |
| `accelLevel / maxAccelLevel` | 精确初始等级或上限，红1蓝2；未声明时不把加速等级默认为0 |
| `bannedIds / allowedIds / maxCounts` | 禁用ID、允许ID列表，以及如`{"7":4}`的数量上限 |
| `preserveIds` | 结构规则中的附加限制；坐标规则即使写false也不能换兵或增删 |
| `dropControl` | 如`{"removeSlot":29,"unitSlot":30}`；accept时移除指定支撑，调用真实静态重力检查另一槽必须下落并由车板承托。此处不模拟攻击或纸壁损坏时间 |

未知规则字段直接拒绝，防止拼错字段却当成约束已经生效。自然语言条件须先落实为可执行审计或独立诊断，不能塞入规则后假定工具会理解。

## 组合、模板与抽样

同group的axes取笛卡尔积，groups取并集并按纯码保序去重。`slots`里的各槽共同采用一个取值；`values`是绝对值，`offsets`是相对父阵值。完整`template`存在时，偏移改为相对模板，模板为包含核心的`[[id,x,y,r],...]`列表；仅用于结构规则。角度轴归一到0–359，坐标越界拒绝，不裁剪。非核心`id`轴只支持结构规则的绝对`values`。

关联改动可用`options`，每个选项是若干绝对赋值；不同axis不能重复写同一slot字段：

```json
{"options": [
  [{"slot":1,"field":"r","value":290}, {"slot":2,"field":"r","value":300}],
  [{"slot":1,"field":"r","value":292}, {"slot":2,"field":"r","value":302}]
]}
```

group可声明`sample`和整数`seed`，按混合进制组合索引无放回抽样，不创建全部笛卡尔积。省略seed时固定为0。`deterministic-index-v1`与临时薄层的“先全排列再shuffle”不保证选中相同子集；已冻结的旧候选直接复用，不重生成替换。未抽样组合及整份计划实际尝试数量各最多1000000；抽样母空间不超过平台整数范围。`maxCandidates`为唯一合法新码的安全限额，范围1–20000；超限整次报错，不发布截断名单。

## 生成、稳定化、验收与对战

PowerShell入口自动激活conda opencode。直接调用Python时先按[环境准备](03-tool-runbook.md#环境准备)激活该环境。

```powershell
.\tools\ai\formation_workflow.ps1 generate --plan experiments/next/plan.json --out experiments/next/generated
```

输出`candidates.json / formations.txt`是**尚未重力审核的内部草稿**。同时保存冻结plan/rules、逐项rejected、保序aliases、父阵与精确改动、预算/加速、时序提示、输入与工具哈希。没有合法新候选时退出码3，保存拒绝证据，不发布空成功名单。所有输出禁止覆盖。

布局本已稳定，可直接使用下面的`gravity.py evaluate`，由其先审后评。需要落稳转换时，先apply，再accept：

```powershell
conda activate opencode
python tools/ai/gravity.py apply --input experiments/next/generated/candidates.json --out experiments/next/applied --timeout 120
.\tools\ai\formation_workflow.ps1 accept --source experiments/next/generated/candidates.json --applied experiments/next/applied --rules experiments/next/generated/rules.json --out experiments/next/stable-candidates.json
```

accept核验apply证据的输入、哈希与工具身份，再以相同rules检查稳定态预算、禁限、固定字段、配对和dropControl。`--exclude 已测候选.json`可重复声明，按纯码保留首次来源。稳定化后不复制原成绩、W/L/D/U/E或outcomes；重新计算新码哈希与改动。输出旁保存拒绝、源阵映射和audit；去支撑反事实另存`.drop-control/`的输入、指纹与原始结果。即使两个源阵归并为同一稳定态，也保留全部来源映射。

```powershell
python tools/ai/gravity.py evaluate --candidates experiments/next/stable-candidates.json --audit-out experiments/next/preflight --evaluation-timeout 1800 -- --opponents benchmarks/standard200/opponents.txt --side 1 --store experiments/next/cache/store.sqlite --out experiments/next/training --threads 32 --timeout 300 --time-limit 1700
python tools/ai/verify_pool.py --run experiments/next/training --candidates experiments/next/stable-candidates.json --opponents benchmarks/standard200/opponents.txt
```

评测进度读取`training/progress.json`，结果核验以`verify_pool.py`为准。2000与10433使用各自冻结池和独立输出目录，复用同一契约的缓存；不更改对手阵码或权重。来源原码按[原阵研究](formation-tuning.md#11-开搜前先充分研究原阵)完成或复用三池诊断，稳定态为新候选重新审核与评测；不继承旧成绩，也不例行给稳定态重复建立三池基线。

## 约束验证范围

生成器检查见[测试入口](../../tests/test_formation_generator.py)：规则执行、模板偏移、保序去重、大组合空间抽样、纯码回编码、预算/加速独立核算，以及真实重力apply→accept→去支撑反事实。验收还须保证旧成绩不转移、别名不丢失、固定字段落稳后仍生效，未过审核不启动对局。工具验证与候选强度评测分别记录；具体通过范围须匹配实际工具/测试哈希，不能把历史成功当作当前构建自动通过。
