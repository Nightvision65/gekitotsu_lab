# 数据契约

## 身份、坐标与输入

- `side=0`为1P，`side=1`为2P；胜负status仍用`1=1P胜/2=2P胜/0=平/-1=帧耗尽`。其他status计E。
- `evaluate_pool --side 1/2`选择物理1P/2P；事件查询side使用0/1，不能混用。`winnerHp`只表示胜者HP，不能据平局或U的该字段推断双方HP；frames为模拟帧数，不是墙钟耗时。
- 原始矩阵forward的p1Index/p2Index分别索引请求p1/p2；reverse已经交换输入，p1Index索引原请求p2、p2Index索引原请求p1，status仍表示物理胜方。候选在请求p1时，reverse的status2才是候选胜，p2Index也不是对手下标。非方阵尤其须按双方纯码与物理侧位核对。
- 阵码核心始终slot0；非核心按原阵码顺序slot1..n。生成实体slot为null，entityId单局唯一、不复用；不同局的相同entityId没有跨局意义。生成实体父单位未知时保留null。
- `formation`坐标为`kit-workbench`；`battle`事件和单位位置为`battle-world`；trajectory同时提供world与opponent-base-relative坐标。不得混用。
  frame0的Rust核心字段尚是相对车板偏移，观察器以base＋core offset转换为world，标注corePositionSource；frame1起使用Core实体world坐标。`coreTargetX/Y`另存引擎瞄准点，的玉（TargetBall，ID42）可能替代瞄准点，不能把瞄准点误当核心实体位置。这些仅是输出转换。
- 纯码格式、base61与raw/工作台偏移维护在[阵码契约](02-formation-format.md)。当前CLI严格拒绝HP后缀、码内空白/中文、非规范角度、越界与首尾`/`，名称可中文；保留原码，不静默清洗。GUI的宽松容器解析不等于当前CLI接受相同文本。
- 比赛预算、禁限、单位顺序与配对/坐标约束独立冻结；字段校验不是比赛合法性证明。

## 稀疏事件

基础模式主要采用**帧末观察**，满血恢复使用轻量HP写点信号，能覆盖同帧掉血又回满的情况。其他同帧变化仍可能抵消，尤其同帧落地并再次起跳；需要精确HP写点和阶段顺序时开启详细模式。基础events最终按frame稳定排序；同帧排列不表示真实攻击先后，不伪造攻击者或击杀归因。

| kind | 含义与保留策略 |
|---|---|
| `half_hp` | 每次从至少50%跌到严格低于50%的区间；快速恢复过滤通过后保留原跌破帧号与episode，恢复后可以再次触发 |
| `quarter_hp` | 首次严格小于25%，每实体一次 |
| `recovered_half_hp` | 已记录的半血区间恢复到至少50%，按episode配对；被过滤的短区间不输出此事件 |
| `recovered_full_hp` | 真实治疗/修缮写点将HP从低于构造maxHp恢复到满血；每次都记录，包括同帧伤害与治疗互相抵消的情况；基础模式healerEntityId未知，详细模式可查来源 |
| `core_first_damage` | 核心首次帧末HP减少，每侧一次；不逐次记录核心受伤 |
| `death` | 实体从存活集合移除；HP≤0标`hp_depleted`，仍有HP标`removed_with_hp`，不推断更具体死因 |
| `launch` | jumpFlag从非1变成1；记录当帧速度与目标onSide，含突击壁/采配等机制，不从速度武断认定触发者 |
| `land` | jumpFlag从1变成非1；保留onSide/jumpFlag和surface、位置，不把地面与敌车落地混为一谈 |
| `board_enemy` | 首次存活且onSide为敌侧、jumpFlag=0；作为登车观测，不等于造成伤害，也不代表所有突击的目标 |
| `spawn` | 战斗初始化之后生成的Ball/Wall；创建帧为首次帧末发现帧，可同帧死亡；召唤来源不猜测 |
| `fort_collision` | 首次norikomi_flg帧；累计collisionFrames保留，后续每帧碰撞不输出事件 |
| `base_destroyed` | 车板从存在变成不存在，每侧一次 |
| `base_acceleration_changed` | 车板axl变化，fromAxl/toAxl为战斗值；不要将瞬时速度当初始加速等级 |

普通兵玉、墙、加速器、核心都在units内。`initialHp/maxHp/initialCounter/attackCycleBase`取实际Rust初始化状态；attackCycleBase是引擎speed字段而非所有兵种的完整发射周期。单位记录首次里程碑、半血区间/满血恢复/短区间过滤次数、突击/落地次数、出生/死亡帧、最终HP和位置。`hpAtDeath`保留移除时HP；Core在judge终止时可能将负HP钳位到0，`finalHpAdjustment`单独标注，不能当治疗或回满。

`healthEventPolicy=recovery-episodes-v1`：默认`healthRecoveryWindowFrames=30`。跌破半血后在≤30帧内恢复到至少50%则过滤；持续超过窗口、低血移除或低血终局则确认事件，frame仍是实际跌破帧，detail另存confirmedFrame/confirmation。终局低血且尚未满窗口标为battle_ended_below_half，未来恢复未知。恢复但没有回到50%不会开启新半血区间。`firstHalfObservedFrame`是不经过滤的首次帧末观测，`firstHalfFrame`是首个保留事件，不能混用。`recordHalfHp=false`关闭半血、四分之一血与恢复半血记录，满血恢复与死亡继续保留。窗口只是整数帧比较，没有sleep或延迟引擎。

## 突击位置与战术意图

突击可以用于登阵，也可以用于有意落地的地刺辅火、采玉配合等战术。**不预设登阵为成功目标，不将ground落点统一判为失败。** 落点按本轮战术意图解释。

`flights`每次空中段一行。记录originSurface、起跳帧/位置、落地帧/位置、终止帧、durationFrames、endReason、destinationSurface、aliveAtEnd以及终点相对双方核心/车板的坐标。正负x使用全局world方向；2P不偷偷镜像相对坐标。ground对应jumpFlag2、平台对应jumpFlag0与onSide；仍在空中/空中移除没有落地帧。刚生成就在空中者标born_airborne，不假装观察到墙触发。

同帧落地并被采玉再发射可能在帧末仍为jumpFlag1，基础flights无法识别这个中间落点。详细combatEvents在移动与控制阶段分别记录launch/land，并记录已知触发对象及来源单位，可以展开此情况。相同实体多次起跳分开保存；基础模式不从速度倒推触发者。

`cohort.landingSurfaces`按ground/own_platform/enemy_platform/未落地终止原因计次数；`landingPositionStats`对已落地轨迹按surface分别计算worldX/Y、enemyCoreRelativeX/Y的min/mean/max。均值不能代替多峰分布，必要时分页查询flights原始位置；未落地轨迹不计入落点均值。

`netDamageObserved/netHealingObserved`累加帧末净差分，不能认作逐击精确输出或伤害归因。`initialHp`是构造数据，生成实体的initialHp是首次可观测值。catalogHp、工作台对象HP与战斗构造分别标注；当前catalog-hp-v1蓝加速器100、电梯壁75，构建核验见[HP规则](unit-hp-rules.md)。

默认快照间隔120帧，总是包含frame0与终态。快照提供核心HP/位置、车板位置/速度/加速度、存活Ball/Wall数量、空中单位数和activeAttackObjects。后者是传感器内攻击对象存量，不是发射次数、准确命中次数或真实弹丸净伤害。

普通实体事件超过20000会计droppedEvents，summary.complete=false；不可宣称完整遥测。全量结果仍可保存供排错；query与verify不自动把完整性不足当成成功强度证据。

## 分析与评分

胜率字段`scorePercent`采用[timeout-as-draw-v1](10-benchmark-usage.md#评分与晋级)，E>0不赋正常主分；N为声明阵池完整局数。JNI evaluate检查返回矩阵长度，正反侧分别保存。反侧应以候选做2P单独计算，不能把两侧未经说明合并。

贡献表的slots是原阵码槽位；正deltaPoints表示删后更好。保留组合纯码和预算差，不把单删收益升级成全局单位价值。matchup新sensitivity/acceleration使用完整子集分母；guiReport/strengthSummary仍保留旧辅助口径。

战场原GUI metas非核心从index0起、核心放在最后；AI slots显式映射核心slot0/非核心index+1，同时保留reportIndex。damageTaken是受创总量，deathCount是死亡总量，未分组；时间线按group=0赢/1输/2其他分组，提供sampleCount和均值，晚期长局样本不等于整个阵池。

### GUI聚合数组

`rawGuiReport.slotDamage/slotDeaths`表示己方槽位受创/死亡，不是攻击者输出排名。metas非核心k对应阵码slot k+1，核心位于末尾n，不能把数组0当核心；类型、位置、角度、core标志不能唯一对应时保留报告索引，不猜测映射。`S_CORE_DAMAGE_*`与普通单位`S_DAMAGE_*`分开取，标量每局10项，对照`BattlefieldAnalyzer.S_*`常量。

原timeline按胜/负/其他、时间箱、7列展开：双方核心HP各自总和、双方存活单位数各自总和、双方在场弹幕数各自总和、样本数。均值用本箱sampleCount；合并组先求和再除总样本，不能平均各组均值。bin-frames只改聚合粒度，弹幕存量不等于发射或命中序列。

## 存证与缓存

schemaVersion：`gkt-ai-analysis-v1`、`gkt-ai-battle-v1`、SQLite `gkt-store-v1`。每个artifact为冻结请求、后端二进制SHA256、Python适配器SHA256和评分政策的完整SHA256；JNI另记录实际固定加载的DLL SHA256。种子、侧位、帧数、快照间隔、半血策略、mode、详细事件上限与采样间隔、plain状态均冻结。详细存证另记录基础观察器与只读钩子生成器SHA256；两种模式分别缓存。

执行工作目录隔离；Java显式加载当前JAR提取的DLL，禁止在AI接口失败后回退Java。进程有墙钟timeout；失败不生成成功artifact。成功原始JSON先以不可覆盖方式发布，再事务写入索引；可重用完整同请求证据，查询/复用先核对raw SHA256与请求身份。

SQLite `artifacts`存元数据和摘要；`records`存有序具名记录，按artifact/table/side/slot/kind/frame建立索引。`verify`进一步比较数据库每行、行数、表和摘要与raw证据。它核验存证一致性，不代替预算审计或E=0晋级检查。

详细模式的combatEvents/objects/unitStats/damageSources/damageRootSources/healingSources/flightSamples分别索引；entityId/attackerEntityId/sourceObjectId有额外索引，可按攻击者或攻击对象查询。具体语义见[详细模式](detailed-mode.md)。

## 上下文预算

CLI默认只输出artifact及summary；每次query先选择table和字段。默认20行，1..100；默认12000字节，可设512..32768。UTF-8以字节计数，完整JSON不会被截断。字节预算和行数都可能提前结束分页，以实际nextOffset续读。单行过大时返回空rows和字段投影提示。

`diagnose`保存manifest。`cohort`默认20槽位，字节预算可能减少实际行数；以nextOffset续读。均值只计算已死亡/已达到半血/已落地者并具名说明；未达到者保留计数，不当作0帧。不能用筛选损失局的deathRate解释全体对手的胜率。

`compare`要求同指纹、同帧上限、seed、快照设置及冻结对手；仅对同slot同type比较，增删/调序/复活的实体必须人工对应。同 seed 也不保证不同阵的随机消耗顺序一致。
