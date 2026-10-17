# 要塞核心（core_type 0）

普通胜负目标，兼作兵玉支撑与弹体接触对象，没有主动攻击。

## 身份与范围

标准200中199阵使用；slot0编码类型0，工作台目录ID0、军资0、初始HP100。固定Rust提交149e365221b09330a72d9a6a1804605e3ce4e1f5，类型CoreKind::Normal。本页是源码机制；实战结果按冻结引擎另行核验。

## 详细机制、恢复与死亡

核心由阵码slot0的首字符单独解释。core_type=1不是非核心ID1（弓玉）；工作台BOSS目录ID为61。核心不套普通兵玉的x初始化计数公式。标准200只出现core_type=0和1，未收录未出现的狂暴BOSS core_type=2。

两个类型初始game.hp=100；每次Core::step随车体更新位置，以13个采样点检查敌Atk，命中扣1HP，随后若HP<100且核心中心命中repair则+1。不读取heal。核心实际接触/支撑形状另由Core::hit_test_point定义，不等于13点受创判定。

车体相撞或撞边可能独立扣核心HP，贽玉能影响这些碰撞路径。judge在每次update前处理HP耗尽，进入hp0流程后移除车体与核心并等待终局；终局负HP归零是钳位，不是治疗。恢复可让累计承伤超过100。

工作台核心raw=(x+52,y+58)，1P初始战斗中心=(x+102,y+190)，2P=(1818-x,y+190)。核心r不参与这里的Core构造。默认seed含双方核心坐标乘子，移动核心同时改变几何和随机序列。

## 支撑形状与实际落点

`Core::hit_test_point`先限制相对坐标在x/y各±53.5以内，再取a=abs(dx)、b=abs(dy)。碰撞形状为以下区域的并集：中心半径33.5的圆；a<40且abs(b-a)<=9.221316的对角臂；四个圆心为(±40,±40)、半径13.5的角圆。它不是外接矩形，也不是单一圆，且与核心受伤使用的13个采样点是两套判定。

普通兵玉以脚点探测这一形状，因此同一x可有不同支撑层，相邻x也可能跨过角圆或对角臂边缘而落向车板。初始y决定首先遇到哪层；核心移动还会同时改变其他兵玉支撑与默认seed。几何公式适合生成有依据的候选，最终仍须调用实际Ball::land做稳定态审核，不能以近似形状代替审核。

## 改阵与对手诊断

x/y决定核心相对平台、护壁、修理覆盖、攻击线路和被突击接近的距离。实际核心位置与core_x/core_y追踪目标字段区分：的玉可以替换后者，不能替代真实核心承伤。优先检查首次受创与后续来源、repair次数、碰撞扣血、保护结构死亡和默认seed耦合。

源码：[core.rs](../../../../rust/vendor/gkt-core/src/engine/core.rs)的Core::step/hit_test_point/kill；[mod.rs](../../../../rust/vendor/gkt-core/src/engine/mod.rs)的new/main_setup/base_move/judge/atk_hit_core13；[decode.rs](../../../../rust/vendor/gkt-core/src/decode.rs)的core_spawn/seed_factor；[base.rs](../../../../rust/vendor/gkt-core/src/engine/base.rs)的Base::step。

[共通规则](common.md) · [缮玉](17-缮玉.md) · [的玉](42-的玉.md) · [单位索引](README.md)
