<p align="center"><a href="../../README.md">English</a> · <strong>简体中文</strong> · <a href="README.es.md">Español</a> · <a href="README.ar.md">العربية</a> · <a href="README.hi.md">हिन्दी</a> · <a href="README.fr.md">Français</a></p>

<p align="center"><img src="../media/hero.png" alt="Petal" width="100%"></p>

# Petal

**右键点击文件，选择格式，完成转换。**

Petal 是一款原生 Windows 11 工具，将图片、音频、视频和文本文档转换整合到紧凑的环形菜单中。它支持文件资源管理器和桌面，跟随 Windows 主题与强调色，并可通过中央按钮打开 Windows 原生右键菜单。

本页提供简体中文说明。**应用界面目前为英文。**

## 动态预览

![Petal 的环形菜单、悬停动画、进度指示和完成标记](../media/demo.gif)

预览由原生渲染器生成，使用示例文件名。[视觉素材说明](../MEDIA.md)（英文）。

## 功能

- 最多六个相关格式选项；当前格式会被替换为其他选项。
- 在本地转换图片、音频、视频及文本文档，不上传文件。
- 浅色和深色主题、Windows 强调色、花瓣动画，以及减少动态效果支持。
- 加载动画、完成对勾、带品牌图标的 Windows 通知和可关闭的提示音。
- 使用 Rust 和 Win32/GDI+ 构建，无浏览器运行时。

> **原始文件保持不变。** Petal 验证转换结果后，在同一文件夹中以新扩展名保存副本，例如 `photo.jpg` → `photo.webp`。绝不覆盖已有目标文件。转换副本的元数据或排版可能发生变化。

## 格式

| 选中的文件 | 默认选项 | 当前格式的替代选项 |
| --- | --- | --- |
| 图片 | JPG · PNG · WebP · GIF · BMP · PDF | TIFF |
| 音频 | MP3 · WAV · FLAC · M4A · OGG · MP4 | OPUS |
| 视频 | MP4 · MKV · WebM · MOV · MP3 · WAV | AVI |
| 文本文档 | TXT · DOCX · MD · HTML · RTF · PDF | ODT |

音频转 MP4 会生成带原有声音的黑色背景视频；视频转 MP3 会提取音轨。文档转换保留文字与段落，而非原始排版。扫描版 PDF 需要 OCR，当前不支持。参阅[格式与限制](../../FORMATS.md)（英文）。

## 开始使用

**系统要求：Windows 11 x64。**

### 下载构建产物

打开一次成功的 [Windows 工作流运行](https://github.com/Syawi/Petal/actions/workflows/windows.yml)，在 artifacts 中下载 **Petal-Windows-x64**。需要登录 GitHub。完整解压文件夹，然后运行 `Petal.exe`。

核心构建支持图片与文档。要启用音视频转换，请在该文件夹中使用 PowerShell 运行一次附带的脚本：

```powershell
./Install-MediaEngine.ps1
```

将 `assets/` 和安装后的 `ffmpeg/` 保留在 `Petal.exe` 旁。不需要管理员权限或 Office。应用会为当前用户注册开始菜单快捷方式和通知标识，但不会自动加入开机启动。

### 从源码编译

安装 [Rust](https://rustup.rs/)、Visual Studio C++ 生成工具和 Windows SDK（MSVC），然后执行：

```powershell
git clone https://github.com/Syawi/Petal.git
cd Petal
./Build.ps1
./Petal.exe
```

构建脚本准备 FFmpeg/FFprobe、校验下载的 SHA-256、运行测试并生成可执行文件。GNU 构建说明见 [CONTRIBUTING.md](../../CONTRIBUTING.md)（英文）。

## 使用环形菜单

1. 启动 `Petal.exe`，蓝色图标将出现在系统托盘中，也可能位于隐藏图标区域。
2. 在文件资源管理器或桌面上右键点击一个受支持的文件。
3. 点击目标格式。Petal 创建并验证副本，然后显示 **Conversion complete**。

中央的 **Windows menu** 打开原生右键菜单。按住 **Shift 再右键点击**可直接使用 Windows 菜单。点击菜单外部或按 **Esc** 可隐藏菜单；正在进行的转换会在后台继续。

| 快捷键 | 操作 |
| --- | --- |
| `1`–`6` | 使用对应的花瓣选项转换 |
| `Tab`，然后 `Enter` / `Space` | 选择并启用选项 |
| `M` | 打开 Windows 右键菜单 |
| `Esc` | 隐藏菜单或反馈动画 |

托盘菜单中的 **Mute sound** 用于关闭提示音，**Quit Petal** 用于退出。提示音设置会被保存；转换期间无法退出。

## 外观与开发

![Petal 的 Windows 浅色与深色主题](../media/themes.png)

悬停时，花瓣或中央按钮会填充强调色，文字变为黑色。启用减少动态效果后，应用关闭位移动画并使用静态反馈。

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

Windows CI 检查格式、运行测试并构建应用。交互式桌面检查和 README 帧导出需手动启用。英文技术文档：[测试](../../TESTING.md)、[贡献指南](../../CONTRIBUTING.md)、[视觉素材](../MEDIA.md)。

## 许可证与致谢

[MIT 许可证](../../LICENSE)，版权所有 © 2026 Syawi。第三方工具与素材保留其各自许可证：[第三方声明](../../THIRD_PARTY_NOTICES.md)、[素材来源](../../assets/SOURCES.md)、[媒体引擎来源](../../ffmpeg/SOURCES.md)。
