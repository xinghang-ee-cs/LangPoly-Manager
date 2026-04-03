//! 进度显示样式模块。
//!
//! 本模块定义统一的进度条和 spinner 样式，确保整个项目 UI 风格一致。
//! 使用 "🌙 月亮" 主题，提供友好的视觉反馈。
//!
//! 主要函数：
//! - `moon_spinner_style()`: 生成月相风格 spinner（用于不确定时长的操作）
//! - `moon_bar_style(template)`: 基于模板生成月相风格进度条（用于已知总进度的操作）
//!
//! 设计元素：
//! - **Spinner 帧序列**: 月相变化 8 帧 + 满月重复，共 9 帧
//!   `["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘", "🌕"]`
//! - **进度条字符**: 从全块到空块的 9 级渐变
//!   `"█▉▊▋▌▍▎▏ "`
//! - **颜色**: 默认使用终端前景色，可通过 `ProgressStyle` 自定义颜色
//!
//! 使用场景：
//! - `moon_spinner_style`: 下载中（未知总大小）、安装中（无法预估步骤）
//! - `moon_bar_style`: 下载进度（已知文件大小）、安装步骤（已知总步骤数）
//!
//! 示例：
//! ```rust
//! use indicatif::ProgressBar;
//! use meetai::utils::progress::{moon_bar_style, moon_spinner_style};
//!
//! let spinner = ProgressBar::new_spinner();
//! spinner.set_style(moon_spinner_style());
//! spinner.set_message("正在下载...");
//! spinner.finish_with_message("下载完成");
//!
//! let bar = ProgressBar::new(3);
//! bar.set_style(moon_bar_style("{wide_bar} {pos}/{len} ({percent}%)"));
//! for _ in 0..3 {
//!     bar.inc(1);
//! }
//! bar.finish_with_message("完成");
//! ```
//!
//! 模板变量（`moon_bar_style`）：
//! - `{spinner}`: spinner 动画（同 `moon_spinner_style`）
//! - `{wide_bar}`: 完整进度条（包含已完成的块和空块）
//! - `{bar}`: 进度条（不含两端边界）
//! - `{pos}`: 当前进度值
//! - `{len}`: 总进度值
//! - `{percent}`: 百分比（0-100）
//! - `{elapsed_precise}`: 已用时间（精确到毫秒）
//! - `{msg}`: 自定义消息
//!
//! 注意事项：
//! - 所有样式通过 `ProgressStyle::with_template` 构建，模板字符串必须有效
//! - 终端需支持 Unicode 字符（现代终端均支持）
//! - Windows 传统 `cmd.exe` 可能需要调整代码页（`chcp 65001`）
//!
//! 测试：
//! - 样式构建不 panic（模板有效）
//! - tick 字符串长度正确（9 帧）

use indicatif::ProgressStyle;

const MOON_TICKS: &[&str] = &["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘", "🌕"];
const MOON_PROGRESS_CHARS: &str = "█▉▊▋▌▍▎▏ ";

/// 生成统一的月相风格 spinner 样式。
pub fn moon_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner} {elapsed_precise} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .tick_strings(MOON_TICKS)
}

/// 基于调用方模板生成统一的月相风格进度条样式。
pub fn moon_bar_style(template: &str) -> ProgressStyle {
    ProgressStyle::with_template(template)
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .tick_strings(MOON_TICKS)
        .progress_chars(MOON_PROGRESS_CHARS)
}
