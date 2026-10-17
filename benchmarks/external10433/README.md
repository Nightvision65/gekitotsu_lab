# 冻结10433对手集

本目录分发固定对手输入及其可复核清单，不包含维护者的搜索候选、对局结果、下载路径或本地原始快照。opponents.txt含10433个唯一纯码；按原顺序、等权使用，BOSS核心同样保留。

manifest.json记录规范输入SHA256、条数与格式；evaluation-partitions.json和standard200-overlap.json记录与standard200的精确纯码重合。198条重合，10235条不重合；编码不重合不等于阵族独立。statistics.json提供单位成本与核心类别统计，duplicates.json记录重复组。

从仓库根目录验证随仓库分发的输入，不依赖外部原始文件：

```powershell
conda activate opencode
python benchmarks/external10433/summarize.py
```

脚本只读核验文件SHA256、阵数、唯一性、重合索引及预算/核心统计；失败返回非零。变更阵池应另建版本，不能覆盖既有实验的冻结输入或继承旧成绩。评分与池角色见docs/ai/10-benchmark-usage.md。
