# 构建与发布gekitotsu_lab

源码仓库仅保存可分发源码、测试输入、通用知识与固定参考JAR。GUI输入、个人配置、缓存、实验记录和运行时不进入公开Git历史。运行时随Windows便携包生成，不放入普通Git。

本地持续维护材料按用途放在Git忽略的`local-maintenance/`：修复与版本对照工具、维护记录、具体案例、宣传素材各用独立子目录，可以持续更新。公开文档只描述当前机制、操作与兼容边界，不依赖该目录中的文件；当前功能需要的源码和可复用测试仍随仓库分发。

Lab版本为0.6.5，维护在pom.xml与Version.LAB_VERSION；内置Kit版本为1.9.1，维护在Version.VERSION。界面、更新检查、发布标签与附件使用Lab版本，Kit接口另报kitVersion，不随Lab发版改动Kit版本。

## 本地验收与打包

准备JDK21+、Maven3.9+、Rust（edition 2024、MSVC链接工具）及conda opencode。命令从仓库根执行：

```powershell
conda activate opencode
./tools/ai/build.ps1
python tools/ai/gravity.py build --timeout 300
./tools/ai/test.ps1
python benchmarks/external10433/summarize.py
python tools/release/audit_public.py --revision HEAD
./tools/release/package.ps1 -SkipBuild
```

依赖已缓存时build.ps1与package.ps1可传-Offline；首次联网可用HTTP_PROXY/HTTPS_PROXY显式配置代理，不依赖固定端口或维护者工具路径。缺少PATH时用-Maven/-Cargo指定工具。脚本设有实际进程超时，已有发布文件拒绝覆盖。

dist生成源码ZIP、Windows x64便携ZIP和SHA256SUMS。打包仅从当前干净提交导出公开文件，追加明确的构建产物，不复制工作目录中的GUI输入、配置或缓存。便携包1P.txt与2P.txt始终为空；benchmarks和rust/fixtures是公开固定测试输入，可以含阵码。release-manifest.json记录提交、文件哈希、后端规则与打包冒烟检查。

## 后端与测试边界

研究版Java与Rust观察器采用catalog-hp-v1；继承的JNI DLL仍是旧规则且缺少公开JNI构建工程。两者分别标记身份；默认集成验收核对plain/basic/detail三者，另外执行JNI接口而不声称跨规则逐局一致。`tests/verify_integration.py --compare-jni`保留严格对照模式，只能在规则一致时使用；不一致会失败。旧JNI不能作为新规则的等价证明。未公开的大金样可能跳过，日志中的跳过项不能计作通过。

内置JNI与固定参考JAR的当前版本、规则和SHA256见`app/kit-manifest.json`。Rust发布构建重映射源码与Cargo缓存路径，避免构建机用户目录进入二进制。

## 发布边界

先检查源码ZIP与便携ZIP的清单、空白GUI输入、LICENSE及第三方许可，再核验SHA256SUMS。公开仓库为Nightvision65/gekitotsu_lab，自动更新读取该仓库的最新release。

只推送经过audit_public核验的公开分支与明确的新标签；不要使用--all、--mirror或批量推送本地旧标签。release标签必须指向核验过的公开提交。发布脚本和CI只生成附件，不自动推送或发布GitHub release。
