# BOSS要塞（core_type 1）

带主动旋扫激光的核心，仍以核心HP作为胜负目标。

## 身份与范围

slot0编码类型1、工作台目录ID61、军资750、初始HP100；固定Rust类型CoreKind::Boss。机制依据提交149e365221b09330a72d9a6a1804605e3ce4e1f5；存在于基准不代表本轮比赛允许使用，合法性按rules审计。

## 核心受击、恢复与死亡

核心由阵码slot0的首字符单独解释。core_type=1不是非核心ID1（弓玉）；工作台BOSS目录ID为61。核心不套普通兵玉的x初始化计数公式。标准200只出现core_type=0和1，未收录未出现的狂暴BOSS core_type=2。

两个类型初始game.hp=100；每次Core::step随车体更新位置，以13个采样点检查敌Atk，命中扣1HP，随后若HP<100且核心中心命中repair则+1。不读取heal。核心实际接触/支撑形状另由Core::hit_test_point定义，不等于13点受创判定。

车体相撞或撞边可能独立扣核心HP，贽玉能影响这些碰撞路径。judge在每次update前处理HP耗尽，进入hp0流程后移除车体与核心并等待终局；终局负HP归零是钳位，不是治疗。恢复可让累计承伤超过100。

工作台核心raw=(x+52,y+58)，1P初始战斗中心=(x+102,y+190)，2P=(1818-x,y+190)。核心r不参与这里的Core构造。默认seed含双方核心坐标乘子，移动核心同时改变几何和随机序列。

## 激光的触发与生成对象

核心cnt从0开始每次step加1，在cnt<2761且cnt%900=60时创建激光，即60/960/1860/2760这四个计数点；之后不再创建新束，已经存在的束并不因此自动停止。

Boss Laser初始朝向90°，随后每次follow：1P减0.1°，2P加0.1°；起点跟随核心沿当前方向偏移38。thickness=30、端点圆半径15.5。每次沿方向步进10、最多1000次，到敌fort、Shield或边界停止，形成线带与末端圆Atk。它不依赖编码r来控制初始射角。

Boss激光没有普通定时寿命；本侧hp0后在follow中清理。多个创建时刻产生的束可以同时存在，不能把900理解为一束仅持续900。射线阻挡与受害目标的Atk采样分开，不按普通弹丸迎击规则处理。

## 改阵与对手诊断

核对每束开始时刻、旋扫方向、阻挡位置、核心实际高度和存活。墙/护壁可能改变射线终点；靠静态初始角度不能推断整场扫过区域。核心x/y还影响默认seed，费用与普通核心不同，变更前核对赛制和预算。

源码：[core.rs](../../../../rust/vendor/gkt-core/src/engine/core.rs)的Core::step；[laser.rs](../../../../rust/vendor/gkt-core/src/engine/laser.rs)的Laser::new_boss/follow/step/hit_test_point；[mod.rs](../../../../rust/vendor/gkt-core/src/engine/mod.rs)的main_setup/spawn_boss_laser/judge。

[共通规则](common.md) · [普通核心](core-0-要塞核心.md) · [单位索引](README.md)
