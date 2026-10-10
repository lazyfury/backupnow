# backupnow

把目录用 **git** 备份到外置盘：盘上放一个 **bare 仓库** + 一份 **自描述清单**，单端即可完成，不需要第二个"设备"。

## 设计

```
本地（源目录，CLI 用 --work-tree 指向它）
~/.pi/
├── backupnow_sync.json        # 清单：source / restoreDir / ignore（tracked）
└── agent/ ...

盘上
/Volumes/数据/Backups/
└── pi.git/                    # bare 仓库
    ├── backupnow_sync.json    # 同名松散副本（供扫描）
    ├── info/exclude           # 由清单生成的忽略规则
    └── objects/ refs/ ...
```

- 仓库保持 `bare=true`、CLI 每次显式传 `--work-tree`，所以 `repo.git` 是**纯净 bare 仓库**，将来可直接 `git clone` / 网络服务
- 清单同时存在于**源目录**和**仓库目录**，两边内容一致
- `backupnow list` 直接扫 `<dest>/*/backupnow_sync.json`，无全局注册表

## 两种模式

| | `snapshot`（默认，普通目录） | `mirror`（源目录本身是 git 仓库） |
|---|---|---|
| 盘上仓库 | 快照仓库 | 源仓库的**镜像**（全部 refs + 完整历史） |
| `init` | `git init --bare` + 首次 commit | `git clone --mirror <src> <drive>/<name>.git` |
| `save` | `add -A` + `commit` | `git fetch --prune '+refs/*:refs/*'`（增量、快） |
| `restore` | `checkout` | `git clone <drive>/<name>.git <target>` |
| 忽略规则 | 生效（`info/exclude`） | 不适用（由项目自己的 `.gitignore` 决定） |
| 备份范围 | 工作区全部文件 | **仅已提交内容**（未提交改动不在内） |

`init` 自动识别：源目录有 `.git` → `mirror`，否则 `snapshot`；也可 `--mode` 手动指定。

## 安装

```bash
# 从 GitHub 直接安装（推荐）
cargo install --git https://github.com/lazyfury/backupnow

# 或本地源码
git clone https://github.com/lazyfury/backupnow
cd backupnow && cargo install --path .
```

安装到 `~/.cargo/bin/backupnow`（确保该目录在 `PATH` 中）。

> 中国大陆网络可先设代理：`export all_proxy=http://127.0.0.1:7890`（`cargo` 走 git 协议时同样需要）。

## 用法

```bash
# 新建库（默认 dest = /Volumes/数据/Backups）
backupnow init pi --path ~/.pi                     # 普通目录 → snapshot
backupnow init proj --path ~/code/proj             # 有 .git → 自动 mirror
backupnow init proj --path ~/code/proj --mode mirror
backupnow init proj --path ~/code/proj --preset macos,xcode,node   # 忽略预设
backupnow init proj --path ~/code/proj --no-source-manifest   # 不把清单写进源目录

# 列出 / 查看状态
backupnow list
backupnow status pi

# 快照提交（add -A + 处理忽略 + commit）
backupnow save pi -m "改了点东西"
backupnow save --all                 # 一次备份根下所有库

# 历史
backupnow log pi -n 10

# 恢复（默认回到清单里的 restoreDir）
backupnow restore pi
backupnow restore pi --to /tmp/restore --rev <commit> --path sub --force
backupnow restore pi --snapshot-first                   # 覆盖前先建本地 APFS 快照
backupnow restore --all --dest /Volumes/数据/Backups   # 恢复根下所有库
backupnow list --drives                                 # 扫描所有挂载卷找库

# 忽略规则
backupnow ignore pi list
backupnow ignore pi add '*.log'
backupnow ignore pi rm '*.log'
backupnow ignore pi edit

# 自检
backupnow doctor pi

# 预览（不写入）
backupnow save pi --dry-run
backupnow restore pi --dry-run

# 补全 / man / Finder
backupnow completions zsh > ~/.zfunc/_backupnow
backupnow man --out ~/.local/share/man/man1
backupnow open pi
```

忽略预设：`macos` / `xcode` / `node` / `python` / `rust`（可逗号组合）。

`doctor` 会检查仓库/清单/源/忽略一致性/xattr，并报告 **全盘访问(FDA)**、**仅大小写不同的路径**、**iCloud 未下载文件**等 macOS 特有问题。

## 忽略规则

优先级从高到低：

1. 清单里的 `ignore` 列表 + 应用默认值 → 写入 `<repo>/info/exclude`
2. 源树里已有的 `.gitignore` → git 自动生效
3. CLI `backupnow init -x PATTERN` 追加

默认忽略：`.DS_Store`、`agent/install`、`node_modules`、`agent/sessions`

> `save` 会自动检测"已被忽略但仍被 git 跟踪"的文件并用 `git rm --cached` 取消跟踪，避免盘上残留。

## xattr / Finder 标签

macOS 的 Finder 彩色标签、quarantine、资源叉等都在 xattr 里，git 不存。backupnow 会把它们采集进一个 **git note**（`refs/notes/backupnow/xattr`，挂在对应 commit 上），**不污染源目录**。

- `save` / `init`：采集工作区文件的 xattr，写入 note（内容没变但 xattr 变了也会更新）
- `restore`：checkout 后自动写回 xattr（失败仅警告）
- 值以 base64 存储，二进制精确往返

```json
{
  "v": 1,
  "entries": [
    { "p": "a.txt", "x": { "com.apple.metadata:_kMDItemUserTags": "<base64>" } }
  ]
}
```

开关与过滤：`backupnow config set-xattr off`，或在配置里写 `xattrIgnore` 排除某些键。

## 配置

`~/.config/backupnow/config.json`（可选）：

```json
{
  "dest": "/Volumes/数据/Backups",
  "ignore": [".DS_Store", "agent/install", "node_modules", "agent/sessions"],
  "xattr": true,
  "xattrIgnore": [],
  "notify": true
}
```

`dest` 解析优先级：`--dest` > `$BACKUPNOW_DEST` > 配置文件 > `/Volumes/数据/Backups`（编译默认）

```bash
backupnow config show                 # 查看配置文件路径与当前解析结果
backupnow config set-dest /Volumes/数据/Backups   # 持久化默认根
backupnow config set-xattr off        # 关闭 xattr 采集/还原
backupnow config set-notify off       # 关闭系统通知
backupnow config unset-dest
```

## 新电脑 / 迁移

盘上每个库都是**自描述**的，插上盘就能恢复，无需预先配置：

```bash
# 方式一：直接把路径丢给命令（推荐，零配置）
backupnow list /Volumes/数据/Backups
backupnow restore /Volumes/数据/Backups/pi.git      # 也可给根目录（库唯一时）

# 方式二：先持久化默认根，之后所有命令都省 --dest
backupnow config set-dest /Volumes/数据/Backups
backupnow list
backupnow restore pi

# 方式三：一次恢复盘上所有库（新机初始化）
backupnow list --drives                       # 先看看哪些卷上有库
backupnow restore --all --dest /Volumes/数据/Backups
```

`list` 会在源路径不存在时提示本机可恢复到的位置（`旧路径 → 新家目录`）。

**家目录自动适配**：`restore` 的默认目标优先用清单里的 `pathTemplate`（`{HOME}/.pi`）。
同机上 `restoreDir` 存在就用它；换机器/换用户名（旧路径不存在）时自动落到新机器的 `$HOME`。

`<name>` 参数支持三种形式：裸名字、仓库目录路径（`.../pi.git`）、备份根路径（库唯一时）。

## 清单格式

`backupnow_sync.json`：

```json
{
  "schema": 1,
  "name": "pi",
  "engine": "git",
  "repo": "pi.git",
  "source": {
    "host": "sukedeMacBook-Air.local",
    "user": "suke",
    "path": "/Users/suke/.pi",
    "pathTemplate": "{HOME}/.pi"
  },
  "restoreDir": "/Users/suke/.pi",
  "createdAt": "2026-10-11T03:29:02+08:00",
  "ignore": [".DS_Store", "agent/install", "node_modules", "agent/sessions"]
}
```

`pathTemplate` 让换机器/换用户名也能算出恢复路径。

## 通知

`save`（包括定时任务触发的）结束时发 macOS 系统通知：成功显示结果摘要，失败显示错误。

```bash
backupnow config set-notify off     # 全局关闭
backupnow save pi --quiet           # 仅本次不发
```

实现见 `src/platform/notify.rs`（macOS 走 `osascript display notification`，其它平台 no-op）。

## 定时任务

平台后端**可替换**：macOS 用 launchd（`com.backupnow.timer.<name>` LaunchAgent）；其它平台走 `UnsupportedScheduler`（提示暂不支持）。加 Linux/Windows 只需实现 `platform::Scheduler` trait。

```bash
backupnow timer install pi --every 1h        # 固定间隔（默认 1h）
backupnow timer install --all --every 1h     # 一个任务覆盖全部库
backupnow timer install pi --daily 03:00     # 每天某时刻
backupnow timer install pi --on-mount        # 盘挂载时触发
backupnow timer install pi --on-volume /Volumes/数据   # 指定卷挂载时
backupnow timer install pi --every 1h --skip-on-battery  # 电池供电时跳过
backupnow timer list
backupnow timer status pi
backupnow timer uninstall pi
```

launchd 任务默认带 `LowPriorityIO` / `Nice=5` / `ProcessType=Background`，不抢前台资源。

调度器实际执行：`backupnow timer run <name> --dest <root>`

- **盘未挂载时安静跳过**（exit 0，不算失败）
- **电池供电时跳过**（除非安装时没加 `--skip-on-battery`）
- 输出写到 `~/Library/Logs/backupnow/<name>.{out,err}.log`
- 手动立即触发一次（调试用）：`launchctl kickstart -k gui/$(id -u)/com.backupnow.timer.<name>`

## FAQ

**我在源仓库里 `git commit` / `git push`，会影响 backupnow 同步吗？**

不会。

- **snapshot 模式**：源目录根本不是 git 仓库，与任何 commit/push 无关。
- **mirror 模式**：
  - 你 `git commit` → 下次 `backupnow save` 用 `git fetch` **自动带上**（增量，只传新对象）。
  - 你 `git push` 到远端 → 不影响备份。`--mirror` 的 refspec 是 `+refs/*:refs/*`，会连 `refs/remotes/*` 一起镜像。
  - 工具**只读**源仓库（只 fetch），从不写、从不 push，所以不会干扰你的 git 工作流。
  - 写入源目录的 `backupnow_sync.json` 是**未跟踪文件**（不想写用 `--no-source-manifest`），且不会被计入未提交警告。

**mirror 模式的未提交改动会被备份吗？**

不会——这是 git 语义。`save` 时若检测到未提交改动，会往 stderr 打警告。

**盘上的 bare 库能当 git remote 用吗？**

能。盘上仓库始终是**标准 bare 仓库**（`core.bare=true`，不写 `core.worktree`），所以：

```bash
# 任意位置 clone 出来查看/恢复
git clone /Volumes/数据/Backups/pi.git /tmp/pi

# 也可以直接 push（当成 remote）
git remote add drive /Volumes/数据/Backups/pi.git
git push drive main
```

注意：**不能**从源目录 `~/.pi` 直接 `git remote add` —— 源目录故意不建 `.git`。
工具本身也不走 remote，而是用 `git --git-dir=<盘> --work-tree=<源>` 直接写。

## 将来接网络

因为仓库是纯净 bare 仓库，任何 git 传输方式都能直接服务，无需改造：

```bash
# SSH（打开 macOS 远程登录即可）
git clone ssh://suke@host/Volumes/数据/Backups/pi.git

# 只读 git daemon
git daemon --base-path=/Volumes/数据/Backups --export-all
git clone git://host/pi.git
```

后续在清单里加可选 `remote` 字段 + `push`/`pull` 子命令即可。

## 结构

```
src/
├── main.rs
├── cli/          参数解析
├── config/       清单与应用配置
├── git/          git 命令封装
├── platform/     调度后端抽象（Scheduler trait + macOS/launchd 实现）
└── commands/     init / save / list / status / log / restore / ignore / doctor / config / timer
```
