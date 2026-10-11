# 桌面 UI 工具链再评估：egui（现状）vs GPUI vs 其他（2026-10-04）

> 触发：判断后续是否把桌面应用迁移到 GPUI 或别的 Rust UI 框架。
> 结论先行：**维持 egui/eframe**；GPUI 与其余候选列入观察，设定明确复评触发条件。

## 一、现状事实

- 桌面应用为原生 Rust + egui/eframe 0.35（无 WebView）；UI 源码规模 **10.7 万行**（约占仓库总代码量一半）。
- 刷新纪律已正确：仅任务运行中 `request_repaint_after`，空闲 CPU ≈ 0；耗时任务走后台线程 + channel。
- 已知短板只有一个方向：**表格/列表虚拟化覆盖不全**（除日志查看器外约 6 处 `ScrollArea` 未虚拟化，大量导入时每帧全量布局）。这是 egui 内可定点修的问题（`show_rows` 模式已在日志查看器验证）。
- 平台优先级（项目既定）：**Linux 第一**（Arch 系基线），Windows 为开发机。

## 二、候选框架对照

| 框架 | 定位与渲染 | 成熟度信号（一手） | 对我们（Linux 优先的分析工作台）的意义 |
| --- | --- | --- | --- |
| **egui/eframe**（现状） | 即时模式；wgpu/glow | 独立 crate、长期演进；本项目已用 | 迁移成本 0；短板可定点修 |
| **GPUI** | 混合即时/保留模式；GPU 加速 | 官方页明确 **与 Zed 强绑定**、贡献须在 Zed 仓内、无独立发版承诺 | 迁移成本高（10.7 万行重写）；无独立稳定版 → 不迁移 |
| **Iced** | Elm 风格；`iced_wgpu`（Vulkan/Metal/DX12/OpenGL/WebGPU） | 官方 showcase 即 **COSMIC 桌面（System76）**，COSMIC 已进入 Pop!_OS 与 Fedora/Arch/NixOS；但 crate 文档自述 "experimental software" | **发行版协同最强**：Arch 生态可见、桌面环境背书；若要换框架，首选 spike 对象 |
| **Slint** | 声明式 DSL（Rust/C++/JS/Python）；多渲染后端 | 版本推进到 1.18，官方强调 **LTS 与商业支持**，覆盖嵌入式/桌面/移动 | 适合"要 LTS、要声明式、要嵌入式"的场合；引入 DSL 与许可模型（GPL/商业）需评估 |
| **Xilem / Vello**（Linebender） | 保留模式 + 自研渲染/文本栈 | 长期研究项目 | 观察项：能力上限高，但未到可押注阶段 |
| **Floem**（Lapce 系） | 反应式；wgpu | 编辑器场景驱动 | 编辑器向，与我们表格/图表工作台需求错位 |
| **Makepad** | 着色器驱动、自绘 | 社区较小 | 观察项 |
| **Dioxus-Native / Blitz** | Web 式（React 心智） | 较新，HTML/CSS 渲染路线 | 与我们"零 WebView"取向冲突，暂不考虑 |

## 三、决策

1. **不迁移**。当前 UI 的已知缺陷（表格虚拟化）在 egui 内定点修复即可，收益/成本远优于换框架。
2. **先修短板**：把 6 处 `ScrollArea` 改为 `show_rows`/分页虚拟化，并补一条"空闲不重绘"的回归测试（现有纪律无测试保护）。
3. **若未来必须换**（复评触发见下）：**第一顺序 Iced**（发行版协同、wgpu 同栈、Arch 可见），**第二顺序 Slint**（需要 LTS/声明式/嵌入式的场景）；先做单页 spike（结果表格页）对比包体、启动时间、空闲占用与 Linux 渲染路径，再决定规模。
4. **GPUI 仅在满足"独立发版 + 公开 Linux/Windows 支持承诺"后**才重新进入候选。

## 四、复评触发条件（任一成立才重启评估）

- egui 出现无法绕开的限制（Wayland 高 DPI/输入法、超大表格 60fps 布局等）；
- Iced 摘掉 "experimental" 自述、或其 Linux/Windows 桌面栈达到我们要求的稳定性；
- 发行版需要"自带桌面工作台"，且要求与基线桌面（CUDA/COSMIC 类）栈一致；
- 产品需要编辑器级富文本/多窗口/协作界面（超出当前"分析工作台"定位）。

> 备注：若未来发行版要自带桌面工作台，UI 选型需与发行版基线一起评估（会牵涉 GTK/Qt/Iced/Slint 与基线一致性），与本仓库桌面应用的决定相互独立。
