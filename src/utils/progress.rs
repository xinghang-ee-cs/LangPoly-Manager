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
