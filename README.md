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

## 安装

```bash
cd ~/Documents/backupnow
cargo install --path .        # 安装到 ~/.cargo/bin/backupnow
```

## 用法

```bash
# 新建库（默认 dest = /Volumes/数据/Backups）
backupnow init pi --path ~/.pi

# 列出 / 查看状态
backupnow list
backupnow status pi

# 快照提交（add -A + 处理忽略 + commit）
backupnow save pi -m "改了点东西"

# 历史
backupnow log pi -n 10

# 恢复（默认回到清单里的 restoreDir）
backupnow restore pi
backupnow restore pi --to /tmp/restore --rev <commit> --path sub --force

# 忽略规则
backupnow ignore pi list
backupnow ignore pi add '*.log'
backupnow ignore pi rm '*.log'
backupnow ignore pi edit

# 自检
backupnow doctor pi
```

## 忽略规则

优先级从高到低：

1. 清单里的 `ignore` 列表 + 应用默认值 → 写入 `<repo>/info/exclude`
2. 源树里已有的 `.gitignore` → git 自动生效
3. CLI `backupnow init -x PATTERN` 追加

默认忽略：`.DS_Store`、`agent/install`、`node_modules`、`agent/sessions`

> `save` 会自动检测"已被忽略但仍被 git 跟踪"的文件并用 `git rm --cached` 取消跟踪，避免盘上残留。

## 配置

`~/.config/backupnow/config.json`（可选）：

```json
{
  "dest": "/Volumes/数据/Backups",
  "ignore": [".DS_Store", "agent/install", "node_modules", "agent/sessions"]
}
```

`dest` 解析优先级：`--dest` > `$BACKUPNOW_DEST` > 配置文件 > `/Volumes/数据/Backups`（编译默认）

```bash
backupnow config show                 # 查看配置文件路径与当前解析结果
backupnow config set-dest /Volumes/数据/Backups   # 持久化默认根
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
```

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
└── commands/     init / save / list / status / log / restore / ignore / doctor
```
