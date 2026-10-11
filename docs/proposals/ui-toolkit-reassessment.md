# 桌面 UI 工具链再评估：egui（现状）vs GPUI（2026-10-04）

> 触发：需要判断后续是否把桌面应用迁移到 GPUI（https://gpui.rs/）。
> 结论先行：**维持 egui/eframe**；GPUI 列为观察项，设定明确的复评触发条件。

## 现状事实

- 桌面应用 `apps/linxira-bio-ui` 为原生 Rust + egui/eframe 0.35，无 WebView；UI 源码规模 **10.7 万行**（占仓库总代码量约一半）。
- 刷新纪律已正确：仅任务运行中 `request_repaint_after`，空闲 CPU ≈ 0；耗时任务走后台线程 + channel。
- 已确认的 UI 短板只有一个方向：**表格/列表虚拟化覆盖不全**（除日志查看器外约 6 处 `ScrollArea` 未虚拟化，大量导入时每帧全量布局）。这是 egui 内部可修的问题（`show_rows` 模式已在日志查看器验证），不需要换框架。
- 平台优先级（项目既定）：**Linux 第一**（Arch 系基线），Windows 为开发机。

## GPUI 的关键事实（一手来源：gpui.rs 官方页）

- 定位为"hybrid immediate and retained mode, GPU-accelerated UI framework for Rust"，来自 Zed/Delta 团队。
- **与 Zed 强绑定**：官方页明确"For the near future, GPUI is tied to Zed, so contributions will need to be made there and kept in sync with it"；文档与组件库都在 Zed 仓内（`crates/gpui`、`crates/ui`），不是独立发版的组件生态。
- 官方页未给出跨平台支持矩阵与稳定版承诺；文档主体在 Zed 仓库内部。

## 决策矩阵

| 维度 | egui/eframe（现状） | GPUI | 判定 |
| --- | --- | --- | --- |
| 发版与稳定性 | 独立 crate、semver、长期演进 | 与 Zed 仓库同步，无独立稳定发版 | egui 胜 |
| 迁移成本 | 0 | 10.7 万行 UI 重写 + 自建表格/图片/3D 组件 | egui 胜（否决性成本） |
| 性能 | 已有问题为局部虚拟化缺失，可定点修 | 渲染架构更现代（GPU 加速、保留模式混合） | 无实测证据表明我们受框架限制 |
| 平台（Linux 优先） | wgpu/glow 跨 X11/Wayland/Windows 成熟 | 平台矩阵未公开承诺 | egui 胜 |
| 组件生态（表格/图片/文本） | 现成且已用于本项目（含结构查看器、SVG 预览） | 需依赖 Zed 内部 UI crate | egui 胜 |
| 叙事价值 | "原生 Rust、零 WebView、空闲零占用" | "与编辑器同款框架" | 两者都可用；egui 的叙事已是事实 |

## 结论与行动

1. **不迁移**。当前 UI 的已知缺陷（表格虚拟化）在 egui 内定点修复即可，收益/成本远优于换框架。
2. **先修短板**：把 6 处 `ScrollArea` 改为 `show_rows`/分页虚拟化，并补一条"空闲 CPU 不守护式重绘"的回归测试（现有纪律无测试保护）。
3. **复评触发条件**（任一成立才重启评估，且先做时间盒 spike，不直接迁移）：
   - egui 在目标平台上出现我们无法绕开的限制（如 Wayland 高 DPI/输入法、超大表格 60fps 布局）；
   - GPUI 独立发版并公开 Linux/Windows 支持承诺与稳定 API；
   - 产品需要编辑器级富文本/多窗口/协作界面（超出当前"分析工作台"定位）。
4. 复评时先做**单页 spike**（例如结果表格页）对比包体、启动时间、空闲占用与 Linux 渲染路径，再决定是否扩大。

> 备注：若未来发行版需要"发行版自带桌面工作台"，UI 框架选择应另立评估（会牵涉 GTK/Qt/GPUI 与发行版基线的一致性），与本仓库桌面应用的决定相互独立。
