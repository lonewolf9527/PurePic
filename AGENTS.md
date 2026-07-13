# PurePic 仓库规则

## Git 工作流

- 本仓库使用 Git，主分支为 `main`。
- `Doc/` 是本地设计资料和参考图片目录，整个目录禁止加入 Git。
- 根目录 `.gitignore` 必须保留 `/Doc/`；禁止使用强制添加绕过此规则。
- Git 的用途是记录每次回答完成了哪些修改，不是按知识点或小步骤拆分提交。
- 一次回答中可以完成多个相关修改，但在回答结束前只创建一次提交；不要每完成一个知识点就提交一次。
- 如果本次回答没有产生文件修改，不创建空提交。
- 开始修改前先检查 `git status`，保留用户已有或无关的修改，只暂存本次回答涉及的文件。
- 提交前必须运行适当的格式化、检查、测试和构建。无法验证时，需要在最终回复中明确说明原因。
- 使用简洁、能概括本次回答内容的提交信息。未经用户明确要求，不得 amend、squash、reset 或重写已有提交。
- 不得提交构建产物、IDE 本地设置、日志、缓存、二进制文件或密钥。

## Rust 环境

- 已安装 Rust，不要重复安装或更新 toolchain。
- 当前已知版本：`rustc 1.94.1 (e408947bf 2026-03-25)`。
- PowerShell 中运行 Rust/Cargo 前显式设置：

```powershell
$env:RUSTUP_HOME='C:\Users\lonewolf\scoop\persist\rustup-msvc\.rustup'
$env:CARGO_HOME='C:\Users\lonewolf\scoop\persist\rustup-msvc\.cargo'
```

- 优先使用本地缓存离线构建，常用验证命令：

```powershell
cargo fmt --all -- --check
cargo check --offline
cargo test --offline
cargo build --release --offline
```

- 若新增依赖不在本地 Cargo 缓存中，必须先说明原因并按权限流程请求联网，不得通过重新安装 Rust 解决。
- Cargo 包名为 `purepic`，Windows 二进制目标名必须保持为 `PurePic`。
- 每次产生代码或构建配置修改的回答，在最终回复前必须成功生成 Release 版本：`target\release\PurePic.exe`。
- Release EXE 是构建产物，不加入 Git。
- 无参数运行 `PurePic.exe` 时应显示内置演示图，方便新会话直接检查界面；传入图片路径时打开指定图片。

## 项目方向

- PurePic 目标平台为 Windows 11 x64。
- 技术栈为 Rust、windows-rs、Win32、WIC、Direct2D、D3D11 和 DXGI。
- 首要性能指标是从启动到第一帧可见图片的时间；目录枚举和缩略图生成不得阻塞首图。
- 必须保持 Windows 11 窗口行为、Per-Monitor V2 DPI、有界任务队列和按字节预算管理的图片缓存。
- PNG/JPEG 是主要格式，WIC 是默认解码路径；专业大图或少见格式按真实需求再增加后端。

## 资源约定

- 工具栏和状态栏图标可以使用外部 PNG 或 SVG 源文件。
- 界面图标优先使用 SVG，位图纹理或需要精确像素效果的资源使用 PNG，程序文件图标使用多尺寸 ICO。
- 开发期资源统一放在 `Assets/`。为保持单 EXE 和快速启动，发布时优先编译或嵌入 EXE，而不是依赖用户机器上的松散绝对路径。
- 外部资源必须有明确许可证，不得直接复制来源不明的商业软件图标。

