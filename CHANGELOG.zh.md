# 更新日志

## 0.9.0

CrateBay 当前聚焦本地容器、镜像、Pod 分组和内置运行时管理。

### 变更

- 移除旧的对话式产品面。
- 桌面导航收敛为镜像、容器、Pod 和设置。
- 文档更新为容器管理方向。
- 内置运行时仍然是主要引擎路径。

### 新增

- 镜像归档命令：
  - `cratebay image export --output archive.tar IMAGE...`
  - `cratebay image import archive.tar`
- 基于 Docker 网络的 Pod 命令：
  - `cratebay pod list`
  - `cratebay pod create NAME`
  - `cratebay pod inspect NAME`
  - `cratebay pod add NAME CONTAINER`
  - `cratebay pod remove NAME CONTAINER`
  - `cratebay pod delete NAME --force`
- 一次性容器执行：
  - `cratebay run IMAGE -- COMMAND...`
  - `cratebay container run IMAGE -- COMMAND...`
- Tauri 后端镜像导出/导入命令。

### 保留

- 容器生命周期命令。
- 镜像列表、搜索、拉取、详情、打标签和删除。
- Volume 命令。
- 运行时状态、启动、停止和预下载。
- 内置运行时资产和应用内置镜像加载。
