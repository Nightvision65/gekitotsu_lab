# 激突要塞战术研究

这套资料以11种常见作战思路为索引，说明攻击路线、核心布局、推进节奏、防区与维持链。单位机制由[单位百科](../units/README.md)追到公开源码；用途和相性问题须结合当前后端重新检验。

## 先查哪一篇

- 读阵名前缀、理解软分类：[标签与分类](labels-and-taxonomy.md)。
- 判断为什么赢、为什么输：[攻防与胜负路径](attack-defense-and-win-paths.md)。
- 解释落点、首发、支撑与换边：[时序、支撑和侧位](tempo-support-and-side.md)。
- 比较类别及类内分化：[相性与变体](matchups-and-variants.md)。
- 研究车底攻击、采配和敌后反向火线：[地铁与后击](subway-and-rear-attacks.md)。
- 查询证据、控制数据与上下文：[研究方法及按需遥测](research-method.md)。

## 11类文档

| 主类 | 作战主轴 |
| --- | --- |
| [狙击](01-sniping.md) | 辅助瞄准与击射火线 |
| [空弹](02-aerial-barrage.md) | 从上方进入的弹道覆盖 |
| [正弹](03-frontal-barrage.md) | 正面防区的持续压制 |
| [地核](04-ground-nuclear.md) | 低位维持与核先配套 |
| [辅火](05-supporting-fire.md) | 推进交换中的辅助火力 |
| [加速](06-acceleration.md) | 压缩远程窗口与碰撞净交换 |
| [圣贽](07-holy-sacrifice.md) | 免碰撞受伤保壁与圣贽续接 |
| [自闭](08-attrition-defense.md) | 防区、治疗、修缮与长局得分 |
| [突击](09-assault.md) | 跨防区位移、落点与留守 |
| [弹突](10-barrage-assault.md) | 远程压力与突击的阶段配合 |
| [科技](11-special-technology.md) | 战/导/忍/激光等特殊生成链 |

## 使用边界

后端、规则与缓存身份见[HP规则](../../unit-hp-rules.md)；评分和阵池选择见[阵集政策](../../10-benchmark-usage.md)。每次研究单独冻结输入与完整性条件。

公开资料不分发维护者的运行现场与原始事件。需要逐事件核验时，按[研究方法](research-method.md)针对当前问题生成少量目标局，并保存自己的冻结身份与证据。

标签与结构不一致时按实际解码及战斗解释，保留历史别名。每个机制解释区分源码规则、事件事实和待验假设；观察到某组件共现或输出高，不等于删除/替换它的胜率收益。

[单位百科](../units/README.md)
