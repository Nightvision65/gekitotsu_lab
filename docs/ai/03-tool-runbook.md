# 工具与环境操作手册

本页保存环境、接口调用与排错。造阵顺序见[研究流程](formation-tuning.md)，接口能力见[能力地图](capabilities.md)，字段见[数据契约](data-contract.md)，分析解释见[分析使用](knowledge/analysis-usage.md)。新候选使用重力门禁；低层battle/evaluate不是绕过审核的搜索入口。

## 环境准备

从工作区根目录执行，所有Python必须先激活conda opencode。未初始化PowerShell时先加载hook：

```powershell
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
(& conda 'shell.powershell' 'hook') | Out-String | Invoke-Expression
conda activate opencode
$env:PYTHONIOENCODING='utf-8'
```

请先安装conda并初始化PowerShell；单独调用conda.bat不能保证改变父shell环境，`kit_lab.py`检查`CONDA_DEFAULT_ENV=opencode`。读UTF-8文本显式`-Encoding UTF8`，中文输出在同次调用设置上面的OutputEncoding；不能因终端乱码改写源文件。Windows PowerShell 5.1执行含中文ps1使用UTF-8 BOM，仅按该文件需要处理，不批量换编码。受限语言模式设置失败时换可正确读UTF-8的工具，不反复失败或绕过限制。

低层KitLab需JDK21+的`javac`，自带JRE不替代编译器；桥接本身不要求Python第三方包。源码构建需Maven3.9+、支持Rust2024 edition的工具链和MSVC链接工具。使用`Get-Command java,javac,mvn,cargo`核对PATH；工具不在PATH时传入实际安装路径。

### 网络与依赖

本地和局域网请求不使用代理；需要代理访问外网时，先确认服务可用，再按命令局部设置HTTP_PROXY/HTTPS_PROXY，用完清除。构建工具默认直连，也可通过这些环境变量显式配置代理；不要假定固定本地端口存在。缺包直接装opencode，pip优先`https://pypi.tuna.tsinghua.edu.cn/simple`，仅缺包/不可用时转境外代理；不为任务改变全局Git代理。

### 超时与后台

可能不自然结束的命令必须有真实进程超时：服务一般20–30秒，已知构建/回归按量180–300秒，批评测按冻结预算；工具yield/wait不是杀进程超时。后台子进程可能继承句柄导致外层等不到EOF，启动与健康检查分开，Start-Process使用Hidden。超时后先独立短命令看进程/端口/输出，再处理；反复发生的脚本特性记录到AGENTS，且该文件最后改。评测超时层次见[后台与续跑](formation-tuning.md#后台评测与续跑)。

## 当前工具分工

| 入口 | 用途与边界 |
|---|---|
| `formation_workflow.ps1`或`.py` | 仅generate/accept，参数见[生成器](generator.md) |
| `gravity.py` | build/audit/apply/verify/evaluate，按[重力政策](gravity-audit.md)先审后评 |
| `evaluate_pool.py` | 固定侧位、缺口分组、缓存与STOP；新候选经gravity包装，未改动来源原码的只读诊断例外按研究流程 |
| `verify_pool.py` | 按候选/对手/物理侧位/原始矩阵行和SHA256独立复核 |
| `ai.py` | 当前研究版图鉴、工作台、JNI评测/分析、观察器、存证及有界查询 |
| `kit_lab.py` / `KitLab.java` | 对冻结发行包的隔离桥接、probe/audit和分析；指纹与研究版不同 |
| `score_200.py` | 对低层标准200矩阵核对输入/顺序/缺漏并汇总，2P单列；不替代合法性审核 |

不存在生成包装器的evaluate/status/gate/rank-pool，也没有training-only/cache-only/final-test选项。进度读progress/summary，逐池门禁使用verify_pool，完整命令维护在[执行流程](formation-tuning.md#评测与逐池核验)。

## 研究版读取、诊断与查询

全局参数如store/timeout/observer放子命令之前，下面输入路径和artifact须替换为实际冻结对象：

```powershell
conda activate opencode
python tools/ai/ai.py --timeout 30 capabilities
python tools/ai/ai.py --timeout 30 probe
python tools/ai/ai.py --timeout 30 catalog
python tools/ai/ai.py --timeout 30 formation --file experiments/my-run/parent.txt
python tools/ai/ai.py --store experiments/my-run/diagnostic/store.sqlite --timeout 180 battle --p1 experiments/my-run/parent.txt --p2 experiments/my-run/selected-opponent.txt --max-frames 65536
python tools/ai/ai.py --store experiments/my-run/diagnostic/store.sqlite query --artifact <基础artifact> --table units --side 0 --slot 0 --limit 20
python tools/ai/ai.py --store experiments/my-run/diagnostic/store.sqlite verify --artifact <基础artifact>
```

文本CLI要求单阵`名称&纯码`或纯码，多阵`/`连接，不带首尾`/`，不在`&`后换行或码内留白。兼容GUI输入时另存规范化诊断输入，保留原码与哈希；规范文件见[阵码契约](02-formation-format.md)。

候选2P的单场诊断将对手放p1、候选放p2，事件query用side1；不是side2。先确认观察器后端/HP规则与评测一致，基础与详细结果范围见[数据契约](data-contract.md)，详细重放操作见[详细模式](detailed-mode.md)。默认20行/12000字节，字段投影、槽位/帧窗/来源筛选后再分页，不把整场raw载入上下文。独立诊断SQLite用独立父目录，避免共用raw发布路径。

`analyze --kind contribution/matchup/battlefield`可能重新模拟，不是默认纯后处理；strength还会增加对手间双边赛。优先复用完整结果离线聚合，仅为明确问题调用。返回表、删除delta方向、热图与时间线解释查[分析使用](knowledge/analysis-usage.md)和数据契约，不以GUI旧分母覆盖当前政策。

## 发行包桥接

`kit_lab.py`在隔离临时目录编译KitLab并调用原JAR公开接口，记录JAR/DLL、输入和桥接代码哈希，不写GUI工作目录。输出必须新路径；Java执行由`--timeout`限制，编译另限30秒，失败不发布成功结果。`"NaN"`表示无有效样本，不当0。

| 命令 | 输入及特性 |
|---|---|
| probe / catalog | 后端和分隔行为核验 / 导出真实目录参数 |
| audit | `--p1`，解码、成本、等级、槽位、边界、回编码；不自动拒绝超资或证明赛制合法 |
| battle | `--p1/--p2`矩阵，可both-sides；默认Rust不回退，显式Java对照独立记录且顺序单线程 |
| contribution | 单一p1对p2，单删默认，穷举非核心（包括墙/加速）；固定100%探索 |
| matchup | 单一p1对p2，可strength；至少6对手才有相应强度分析意义 |
| battlefield | 单一p1对p2，filter为ALL/WIN/LOSE/DRAW/TIMEOUT，空组拒绝；bin-frames仅改变聚合粒度 |

```powershell
conda activate opencode
python tools/ai/kit_lab.py probe --out experiments/demo/probe.json --timeout 30
python tools/ai/kit_lab.py audit --p1 docs/ai/examples/candidate.txt --out experiments/demo/audit.json --timeout 30
```

默认4线程、65536帧、maxJobs10000；扩大任务上限与超时须有预算依据。低层battle与score_200用于既有/诊断证据，正式新候选走重力门禁。三个分析命令要求Rust；Java回退不是同一评测线。单矩阵结果缺失不能当完整缓存。旧桥接的seedPolicy为engine-default，研究版为native-default，保留原值与指纹。

分析前估算新增模拟：n个非核心、m对手时，单删约`(n+1)*m`局，双删约`(C(n,2)+1)*m`局；strength额外对手双边赛的上限为160000局，战场分析通常先矩阵再对选定组遥测、至多约`2*m`。先单删或少量问题对照，再按证据扩大，不默认全开。旧GUI原始数组布局在[数据契约](data-contract.md#原gui聚合数组)维护。

## GUI结构复核与交互CLI

GUI操作正文见[使用手册](../../使用手册.md)。工作台核对预览、顺序、预算和等级，效果执行后保存**输出阵码**并重新审计，不只留参数。经验提示与第一帧挤压近似不是连续重力审核或胜率结论。GUI对战编辑会自动保存根目录1P/2P，AI实验不得覆盖。

| 内置效果 | 需要审计的实际变化 |
|---|---|
| 阵容平移 | 核心和单位一起移动，核心种子及边界裁剪 |
| 顶置的玉 | ID42的y设0 |
| 前置愈缮玉 | 核心后愈、缮优先，其他相对顺序保留；重建Unit可能恢复默认HP |
| 单位替换 | 类型、预算、功能与合法性 |
| 要塞壁随机旋转 | GUI未固定Random seed，保存最终码才能复现 |
| 角度规范化 | 核心/墙角设0，不是所有角度取模 |

`java -jar <绝对JAR路径> --cli`是交互菜单，读进程cwd的1P/2P/config，写result/simple_result；0开始、1帧数、2HP、3重载、4线程、9退出。自动化须隔离目录，输入0后9并设实际超时，不能当作p1/p2参数API。

## 源码构建与显式seed

使用`tools/ai/build.ps1`与`test.ps1`，缺PATH时传-Maven/-Cargo完整路径。默认构建当前派生HP规则，不会重建发行包缺源码的JNI；先核验vendor清单再生成basic/diagnostic副本。工具为子进程设置真实超时，报告使用新目录，不覆盖旧结果。

```powershell
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
conda activate opencode
./tools/ai/build.ps1
./tools/ai/test.ps1 -Benchmark
```

依赖已缓存时构建可加-Offline；首次境外下载按代理规则。当前catalog-hp-v1与旧DLL规则不同，不能删掉失败比较后宣称旧JNI全池等价；区分本轮实际运行、失败、未运行和跳过的检查，兼容范围见[项目地图](01-project-map.md#引擎核验与证据范围)。

需要隔离核验HP或计时机制时，使用对应单元测试，按规则版本设置独立CARGO_TARGET_DIR，并给Cargo子进程180–300秒真实超时：

```text
cargo test --offline --locked --manifest-path rust/Cargo.toml -p gkt-observe --test unit_hp_rules --test unit_timing_knowledge
```

计时测试只推进有效计数、识别实际生成对象，不模拟完整支撑、跳跃或命中；不能把它当完整阵池强度证据。

JNI没有显式seed参数。只有任务需要时另建调用程序，使用冻结gkt-core的`compile_fort → BatchJob → run_batch`或`GameTask::run_single_observed`。`None`使用核心坐标参与的默认种子；`Some(s)`在main_setup后设`s & 0x7fffffff`。同seed列表只控制起始状态，不保证不同阵随机消耗顺序相同，压力测试另列，不代表原游戏随机分布。

原生遥测可用`BattleRecorder::new(TelemetryMode::EventsAndSnapshots, interval)`与`into_telemetry()`；公开workspace没有gkt-jni/HTTP/Python包，不能虚构。gkt-bench输入四列`code1 code2 seed maxFrames`，用于吞吐而非候选排名；PGO、工具链、未公开金样和机器差异按实际核验，不拿上游吞吐承诺本机耗时。
