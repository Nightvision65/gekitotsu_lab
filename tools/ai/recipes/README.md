# 实验复用工具与模板

新实验先查本目录及现有入口，再决定是否需要本地脚本。这里集中实验编排、离线研究和证据核验的可复用部分；不保存具体阵码、私人实验路径、战术结论或自动续搜决策。所有Python先激活conda `opencode`，从工作区根目录运行，中文输出显式设置UTF-8。

## 先复用现有能力

| 需求 | 权威入口 | 本目录补充 |
|---|---|---|
| 阵码、预算、固定字段、配对、批量计划 | [formation_workflow](../formation_workflow.py)、[formation_rules](../formation_rules.py)、[formation_codec](../formation_codec.py) | `templates/coordinate-plan.json`、`structural-plan.json`，占位符须按本轮范围填写 |
| 稳定位置、位移标记、稳定态统一去重 | [stable_dedup.py](stable_dedup.py)、[gravity](../gravity.py) | 全部来源合并去重；`templates/search-expectations.md`记录本轮独立验收条件 |
| 完整阵池、固定侧位、缓存补缺、STOP | [evaluate_pool](../evaluate_pool.py)、[verify_pool](../verify_pool.py) | `experiment.py step`统一保存命令、PID、日志与真实超时；不另写评测引擎或缓存 |
| 对手分群、排名、真实父阵增失分 | [pool_report.py](pool_report.py) | 可重复给出参照run，逐候选读`parentCode`；`templates/groups.json`示例 |
| 单局/画像/热图/贡献/详细重放与查询 | [ai](../ai.py)、[detail_slice](../detail_slice.py) | 根据报告改善/退步索引选少量局，不默认启动全量遥测 |
| 建档、冻结输入、文件与SQLite引用核验 | [experiment.py](experiment.py) | `init / snapshot / verify-snapshot`，复用[STATE及元数据](../../../docs/ai/templates/STATE.md) |
| 找出又被复制的脚本与函数 | [script_inventory.py](script_inventory.py) | 只做文件哈希/静态AST盘点，不导入、不执行旧实验 |
| 子代理短任务与实际token核算 | [协作流程](../../../docs/ai/subagent-workflow.md)、[agent_usage.py](agent_usage.py) | `templates/agent-task.md`；只读明确指定的本地会话计数，不输出正文 |

## 建档与单步运行

```powershell
conda activate opencode
python tools/ai/recipes/experiment.py init --source inputs/parent.txt --rules inputs/rules.json --pool 200=benchmarks/standard200/opponents.txt --pool 2000=benchmarks/validation2000/opponents.txt --pool 10433=benchmarks/external10433/opponents.txt --jar target/gekitotsu_lab-0.6.7.7.jar --out experiments/my-run
```

先按[构建说明](../../../docs/ai/03-tool-runbook.md#源码构建与显式seed)准备研究JAR。`init`必须显式指定`--jar`，先核验研究`--ai`入口、执行有超时的JNI probe，再冻结相同字节为`inputs/engine.jar`；记录`engine-probe.json`、JAR/DLL哈希与工具指纹。探测不运行对战；不支持研究接口或JNI不可用时拒绝建档。

后续`evaluate_pool.py`必须显式使用这个冻结副本，诊断`ai.py --jar`也使用同一文件，不依赖target目录的可变构建。调用会核对副本旁`identity.json`中的JAR/DLL哈希，副本被覆盖即拒绝；评测每批还核对本run引擎指纹。`app/gekitotsu_java-1.9.1.jar`是GUI发行包，不能用于本链；需要只读核对发行包时使用独立的`kit_lab.py`桥接，证据分开。

`init`要求新目录，同时冻结源文件、指定父阵、规则与原顺序原权重的阵池，复制STATE/元数据模板；随后填写本轮预算、可动字段、预期和选拔规则。它只建档和探测，不生成候选或开始基线/搜索。

所有可能较长的步骤可用统一监督器，命令参数在`--`后逐项传入，不经shell拼接：

```powershell
python tools/ai/recipes/experiment.py step --out experiments/my-run/steps/generate-001 --timeout 180 -- python tools/ai/formation_workflow.py generate --plan experiments/my-run/plan.json --out experiments/my-run/generated
python tools/ai/recipes/experiment.py step --out experiments/my-run/steps/evaluate-001 --timeout 3600 -- python tools/ai/evaluate_pool.py --jar experiments/my-run/inputs/engine.jar --candidates experiments/my-run/candidates.json --opponents experiments/my-run/inputs/pool-2000.txt --side 1 --store experiments/my-run/cache/store.sqlite --out experiments/my-run/evaluation --batch-size 4 --timeout 600 --time-limit 3400
python tools/ai/recipes/experiment.py step --out experiments/my-run/steps/verify-001 --timeout 300 -- python tools/ai/verify_pool.py --run experiments/my-run/evaluation --candidates experiments/my-run/candidates.json --opponents experiments/my-run/inputs/pool-2000.txt
```

上述为单步调用格式，生成后先完成稳定态去重与本轮验收，`candidates.json`指最终验收名单，才能运行评测示例；原阵基线遵循[研究流程](../../../docs/ai/formation-tuning.md#11-开搜前先充分研究原阵)。每次step使用新目录，保存`command.json / process.json / output.log / result.json`；超时终止进程树，返回124。默认只接受退出码0；可显式`--accept-exit 3`保存诊断/待查，但仍须读子工具summary。step成功不代替progress与独立verify。后台启动后继续独立研究，不自动恢复STOP或运行依赖尚未研究结果的下一批。

## 稳定态统一去重

```powershell
python tools/ai/recipes/stable_dedup.py run --input experiments/my-run/generated/candidates.json --out experiments/my-run/dedup --timeout 120
python tools/ai/recipes/stable_dedup.py verify --run experiments/my-run/dedup
```

可重复给出`--input`合并多批，或用`--exclude-run`显式排除同身份、同稳定窗口的已有几何类。全部位移通过和未通过来源在同一集合去重，保留输入、原生报告、几何类、别名及代表映射。默认优先保留位移通过原码，否则尝试可编码稳定态并重放验证；负坐标等不可编码几何保留合法来源原码，未收敛才单列待查。输出仍须按[本轮预期模板](templates/search-expectations.md)批量检查规则与任务条件。模型、精度和状态见[重力审核](../../../docs/ai/gravity-audit.md)。

## 离线排名、分群与父阵比较

```powershell
python tools/ai/recipes/pool_report.py --run experiments/my-run/evaluation --opponents experiments/my-run/inputs/pool-2000.txt --reference-run experiments/my-run/baseline/pool-2000 --parents experiments/my-run/candidates.json --groups tools/ai/recipes/templates/groups.json --out experiments/my-run/study-001
```

输出`report.json / ranking.csv / leaders.txt / REPORT.md`。要求来源已有独立`verification.json`且结果SHA256相符，复核完整N、E=0、候选身份与计分；比较须同引擎、物理侧位、seed、帧上限及对手文件字节。报告不重扫大raw，不替代此前的`verify_pool.py`，不新增模拟。

`--parents`逐候选读取真实`parentCode`；参照run可多次指定。没有完整父阵证据就报错，不偷偷换成原基线。整份名单确实共用一个父阵时也可显式`--parent-code <纯码>`。省略两者则只生成排名和画像。精确半分计数排序，同分共列名次；输出逐对手增/失/净计分、百分点、完整变化索引、受益/受损分群与相对父阵的精确改动。

自定义分群为JSON列表：`name`唯一，`anyIds/allIds/noIds`检查兵种存在；`counts`按ID设`min/max`数量；`accelMin/accelMax`设初始加速范围；`core`支持`types/xMin/xMax/yMin/yMax`。同一项条件取交集，不同组可重叠；`all`由工具固定提供，空组N=0、胜率null。核心解码ID为0/61/62。模板分组只作格式示例，复制并在看收益前冻结适合本任务的定义；不把示例阈值升级为普遍战术边界。

## 快照与复核

```powershell
python tools/ai/recipes/experiment.py step --out experiments/my-run-snapshot-step --timeout 300 -- python tools/ai/recipes/experiment.py snapshot --root experiments/my-run --out experiments/my-run-snapshot.json
python tools/ai/recipes/experiment.py step --out experiments/my-run-reverify-step --timeout 300 -- python tools/ai/recipes/experiment.py verify-snapshot --manifest experiments/my-run-snapshot.json
```

停止写入后才快照，清单及监督日志放在被检查目录之外。快照核对目录全部文件、SQLite完整性和artifacts引用的raw哈希；出现WAL/SHM/tmp、文件变化或缺失依赖即失败，不自动修复旧数据。外部raw只绑定路径与哈希，不复制，所以这是完整性快照，不是可搬迁备份；外部依赖保留原位置，或另行按原始哈希复制后重新冻结映射。复核拒绝文件新增、缺失或字节改变，可用`--root`指向搬迁后的目录，但不会重写数据库内的旧路径。

## 盘点重复与扩展边界

```powershell
python tools/ai/recipes/script_inventory.py --root experiments --out local-maintenance/automation-review/inventory.json
```

清单包括脚本路径、时间、大小、SHA256、完全重复文件和函数AST、语法错误。它帮助发现候选复用块，不证明不同脚本语义等价；为冻结构建保留的工具快照重复属于留证，不当作冗余脚本删除。盘点结果、具体案例和历史来源留本地，不加入公开仓库。

新增通用能力时，优先扩展已有入口或这里的模板，参数来自显式配置；可重复的证据与异常检查进入[公开测试](../../../tests/test_experiment_recipes.py)。本地一次性代码仍可留实验，但不跨实验import本地`lab.py/manage.py`。共享代码不默认规划下一批、不混池改权；工具或模板存在不代表本轮原阵研究、去重或预期验收已经执行。
