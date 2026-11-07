//! Lucide glyphs, by the job they do here rather than by their Lucide name.
//!
//! The font is registered as a fallback in every family (see `theme::install`),
//! so these can sit inside ordinary strings and render as icons: no special
//! widget, and no change to how anything is laid out.
//!
//! Codepoints come from `lucide-static`'s `lucide.css`; the font in
//! `assets/lucide.ttf` is from the same package and must be updated with them.

// Navigation and chrome
/// `house`
pub const HOUSE: &str = "\u{e0f5}";
/// `history`
pub const HISTORY: &str = "\u{e1f5}";
/// `book-open`
pub const BOOK: &str = "\u{e05f}";
/// `settings`
pub const SETTINGS: &str = "\u{e154}";
/// `circle-help`
pub const HELP: &str = "\u{e082}";
/// `x`
pub const X: &str = "\u{e1b2}";
/// `ellipsis`
pub const ELLIPSIS: &str = "\u{e0b6}";

// Dictation
/// `mic`
pub const MIC: &str = "\u{e118}";
/// `mic-vocal`
pub const MIC_VOCAL: &str = "\u{e349}";
/// `square`
pub const SQUARE: &str = "\u{e167}";
/// `sparkles`
pub const SPARKLES: &str = "\u{e412}";
/// `wand-sparkles`
pub const WAND: &str = "\u{e357}";

// Actions
/// `share-2`
pub const SHARE: &str = "\u{e156}";
/// `rotate-ccw`
pub const ROTATE: &str = "\u{e148}";
/// `search`
pub const SEARCH: &str = "\u{e151}";
/// `pencil`
pub const PENCIL: &str = "\u{e1f9}";
/// `pencil-line`
pub const PENCIL_LINE: &str = "\u{e4f0}";

// Status
/// `check`
pub const CHECK: &str = "\u{e06c}";
/// `info`
pub const INFO: &str = "\u{e0f9}";
/// `triangle-alert`
pub const WARN: &str = "\u{e193}";

// Settings sections
/// `flask-conical`
pub const FLASK: &str = "\u{e0d5}";
/// `keyboard`
pub const KEYBOARD: &str = "\u{e284}";
/// `globe`
pub const GLOBE: &str = "\u{e0e8}";
/// `clipboard`
pub const CLIPBOARD: &str = "\u{e085}";
/// `lock`
pub const LOCK: &str = "\u{e10b}";
/// `palette`
pub const PALETTE: &str = "\u{e1dd}";
/// `eraser`
pub const ERASER: &str = "\u{e28f}";
/// `timer`
pub const TIMER: &str = "\u{e1e0}";
/// `eye`
pub const EYE: &str = "\u{e0ba}";
/// `folder`
pub const FOLDER: &str = "\u{e0d7}";
/// `chart-column`
pub const CHART: &str = "\u{e2a3}";
/// `archive`
pub const ARCHIVE: &str = "\u{e041}";
