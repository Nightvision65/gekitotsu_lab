# 按需详细模式

先用无遥测evaluate筛阵；对少量有解释价值的对局跑基础battle/diagnose。只有发现值得继续研究的槽位、伤害或突击过程，才从基础artifact详细重放。默认不会将详细模式用于每个候选的200场评测。

```powershell
conda activate opencode
python tools/ai/ai.py replay --artifact <基础artifact>
# 返回新的详细artifact，resultMatchesBaseline必须为true。
python tools/ai/ai.py query --artifact <详细artifact> --table unitStats --side 0 --slot 0
python tools/ai/ai.py query --artifact <详细artifact> --table damageSources --side 0 --slot 0
python tools/ai/ai.py query --artifact <详细artifact> --table combatEvents --side 0 --slot 0 --kind damage --from-frame 1200 --to-frame 1800
python tools/ai/ai.py query --artifact <详细artifact> --table combatEvents --kind attack --side 1 --slot 4
python tools/ai/ai.py query --artifact <详细artifact> --table healingSources --side 0 --slot 3
python tools/ai/ai.py query --artifact <详细artifact> --table flightSamples --side 0 --slot 3 --from-frame 1200 --to-frame 1800
python tools/ai/ai.py query --artifact <详细artifact> --table combatEvents --kind launch --side 0 --slot 3
python tools/ai/ai.py verify --artifact <详细artifact>
```

也可对明确选定的单场battle加`--detailed`直接运行。replay优先，因为它核对当前基础二进制与冻结版本，并以基础raw的完整请求重打；核验status、winnerHp、结束帧、双方HP和最终RNG后才发布。基础二进制已变更时拒绝冒充同版本重放；需先恢复冻结版本或生成新基础证据。

## 实际记录内容

| 表 | 内容 |
|---|---|
| `combatEvents` | 实际攻击对象生成、效果衍生、每个HP写点的damage/heal、核心终止钳位、反射/迎玉拦截、阶段内launch/land/采玉发动等 |
| `objects` | 所有引擎对象的单局ID、kindCode、创建/移除帧、parentEntityId、ownerUnitId、originalOwnerUnitId、rootUnitId；保留已死亡对象 |
| `unitStats` | 兵玉、墙、加速器、核心及生成兵玉的累计实扣/有效失血、治疗、输出、攻击动作/对象数、首次/最后受创、精确阶段内起跳/落地计数、HP账本 |
| `damageSources` | 每个受害单位按当前攻击单位与cause分组：有效损失、原始扣血、事件数、占比及多来源重叠次数 |
| `damageRootSources` | 同一份损失按原始生成链root单位分组，适合追溯反射弹、召唤/复活后的根源；与damageSources是两个视角，不能相加 |
| `healingSources` | 按具体愈玉/缮玉等恢复来源分组；每次heal带hpBefore/hpAfter/restoredFullHp，可识别同帧掉血又回满 |
| `flightSamples` | 仅采样存活兵玉的`jump_flg==1`状态，默认每6帧检查，变化至少0.05 world单位才保存；普通重力下落不一定进入该状态 |

`flightSamples`与launch/land统计围绕`jump_flg==1`，不是所有y变化的完整轨迹。支撑损坏后的普通重力可保持`jump_flg==0`，所以轨迹空表或ownPlatformLandings=0不能否定已观察到的下落。结合稀疏事件/最终坐标、支撑反事实和实际侧位核验；没有逐帧位置时不补写精确落板帧。源码条件见[recording.rs](../../rust/observer/src/recording.rs)与[Ball::land_inner](../../rust/vendor/gkt-core/src/engine/ball.rs)。

基础units/events/timeline/flights仍保留。普通核心虽然不会主动攻击，也有完整伤害/修复统计；BOSS核心主动攻击能追溯到slot0。攻击对象本身的销毁不是兵玉/墙/核心的失血，不混入这些单位的伤害分母。

普通核心接缮玉17的repair，不接愈玉18的heal。反复恢复后，累计有效承伤可超过初始HP；来源百分比的分母是累计有效失血。终局把负核心HP归零属于`finalHpAdjustment`，与治疗分列，不能把这个调整当作缮玉恢复。比较方案时同时读取绝对量、恢复和时长；伤害百分比变大不代表实际承伤更多。

`attack`表示单位在该帧实际创建攻击对象，能区分近战、弹丸、激光与BOSS攻击；持续攻击后续伤害由damage记录。`attackObjects`数发射/命中体数量，`attackActions`按同单位同帧合并，不能把散弹的多弹体当成多轮攻击。没有生成攻击对象的计时/瞄准阶段不冒充有效开火。

`detailSummary.eventPolicy=shield-as-ability-v2`将壁玉创建盾区（objects.kindCode=34）记为`ability`，不计入攻击次数。按实际schema、对象kindCode、ownerUnitId与birthFrame核对盾区，查attack为空不能证明没有开壁；不同schema或二进制的证据保留各自身份。

## 炮玉、爆炸与控制权

炮玉创建子弹时保存parent=炮玉；子弹生成爆炸时保存parent=子弹，并继承owner和root。伤害事件sourceObjectId指向真正命中的爆炸体，attackerEntityId指向具体炮玉，attackerRootEntityId保留生成链根单位。两个相同兵种通过side＋slot或entityId区分。

```powershell
python tools/ai/ai.py query --artifact <详细artifact> --table combatEvents --kind damage --attacker-entity-id <炮玉entityId> --limit 10
python tools/ai/ai.py query --artifact <详细artifact> --table objects --entity-id <sourceObjectId>
# 逐个查询parentEntityId直到炮玉；无需展开全场对象表。
python tools/ai/ai.py query --artifact <详细artifact> --table damageRootSources --attacker-root-entity-id <原始炮玉entityId>
```

反玉反射时保留原parent/root，记录控制权变更与projectile_reflected，后续owner归反玉。每次damage保存当时的控制者；不能用对象终态owner反推它之前所有伤害的归属。召唤或复活产生新entityId、slot=null，与原槽位不自动合并；root视角另行展示生成关系。

## 伤害百分比的分母

引擎对兵玉/墙/核心的许多判定使用多个命中对象的OR：多枚弹同帧重叠仍可能只扣1HP。`engine-first-match-v1`采用**引擎实际先返回true的命中见证**分配这一次损失，并记录overlappingObjectIds与ambiguousHitEvents。它说明本引擎的记账规则，不证明在改变对象顺序或删除另一来源后仍会得到相同伤害。

碰撞、撞边、自我牺牲和墙接触分为fort_collision/boundary_collision/self_sacrifice/wall_collision，无法对应攻击单位时保留空来源与具名原因。所有来源组均保留，不能只展示有名字的攻击者后把比例重新归一化。未知攻击来源计unknownAttackEvents，attributionComplete=false时不能称为全部追溯完成。

百分比以该目标整局的**有效HP损失**为分母：effectiveHpLoss=min(原始扣血,max(hpBefore,0))；过量扣血另存rawAmount。恢复不会抵扣已发生的伤害；累计损失可能超过构造HP。目标没有有效失血时percentage=null。按每个目标核对来源effectiveAmount之和与effectiveDamageTaken，以及占比合计100%。healingSources以实际累计恢复为分母。

核心结束时将负HP钳位到0记hp_adjustment/termination_clamp，不能算治疗。每个单位验收`initialHp - rawDamageTaken + healingReceived + hpAdjustment == finalHp`，不平衡则拒绝发布统计。

## 突击与时间

combatEvents有sequence，保留引擎执行顺序；一个单位同帧移动阶段落地、控制阶段被采玉再发射会有两条事件。触发壁的临时判定对象也有来源墙，采玉发令有具体来源ID；普通掉落不伪造触发单位。每条事件含当时位置、jumpFlag/onSide、速度及phase。

flights是基础帧末空中段；需要展开同帧多段突击时查询详细launch/land序列，不能沿用基础分段数冒充精确数量。unitStats中landingsInStep/groundLandings/ownPlatformLandings/enemyPlatformLandings基于详细阶段事件统计；出生即空中不假装有起跳触发。地面落点不预设失败，判断收益仍看阵的战术意图和胜率。

## 完整性与上下文

默认maxDetailedEvents=500000，允许100..5000000；超上限会明确计droppedEvents，detailSummary.complete和summary.complete变false。累计统计持续覆盖整局，statisticsComplete单独核验HP账本；事件不完整与统计完整必须分开说明。需要完整逐次记录时提高上限重新执行，不能将被截断事件流称为完整。事件上限、采样间隔及mode都参与缓存身份。

先读summary，再读目标单位的unitStats与damageSources；只有解释某一来源或某一阶段时才查combatEvents、对象链或flightSamples。所有表继续使用20行/12000字节默认分页、字段投影、时间/单位/攻击者筛选。完整raw JSON留文件，不一次读入AI上下文。
