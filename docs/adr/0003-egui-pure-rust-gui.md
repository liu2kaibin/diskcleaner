# 纯 Rust GUI：egui，不用 Tauri

产品形态定为 Rust 桌面应用，界面框架选 egui（纯 Rust immediate-mode）而非 Tauri（Rust 内核 + Web 前端）。理由：用户明确要"界面也用 Rust 写"；单 exe 分发；treemap 用 painter 自绘自由度最高；个人工具不需要像素级品牌感。代价是 UI 观感为"专业工具风"、复杂布局开发效率低于 Web 技术。

## Considered Options

- Tauri 2 + React：UI 开发效率更高、观感更现代，但界面层脱离 Rust，且引入前端工具链
- iced / Slint：retained/声明式模型画 treemap 不如 egui painter 直接
