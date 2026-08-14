use ratatui::style::Color;

// Font Awesome 4 glyphs from the Nerd Fonts "fa" set (same PUA block every
// Nerd Font ships, and the block nvim-tree itself falls back to for anything
// nvim-web-devicons doesn't special-case). Written as `\u{..}` escapes rather
// than literal glyphs so the exact codepoint is unambiguous — needs a Nerd
// Font in the terminal to render as a picture instead of a box/blank.
const FOLDER_CLOSED: &str = "\u{f07b}";
const FOLDER_OPEN: &str = "\u{f07c}";
const FILE_GENERIC: &str = "\u{f016}";
const FILE_TEXT: &str = "\u{f0f6}";
const FILE_CODE: &str = "\u{f1c9}";
const FILE_IMAGE: &str = "\u{f1c5}";
const FILE_PDF: &str = "\u{f1c1}";
const FILE_ARCHIVE: &str = "\u{f1c6}";
const GIT: &str = "\u{f1d3}";
const LOCK: &str = "\u{f023}";
const COG: &str = "\u{f013}";
const TERMINAL: &str = "\u{f120}";
const DATABASE: &str = "\u{f1c0}";

const FOLDER_COLOR: Color = Color::Rgb(122, 162, 247);

/// Nerd Font glyph + color for a File Explorer row, mirroring
/// `nvim-web-devicons`' lookup order: exact special-cased filenames first
/// (`Cargo.toml`, `Dockerfile`, lockfiles, dotfiles…), then extension, then a
/// generic fallback glyph — same behavior nvim-tree shows for anything the
/// devicons table doesn't recognize.
pub fn for_file(name: &str) -> (&'static str, Color) {
    let lower = name.to_lowercase();

    if let Some(icon) = special_cased(&lower) {
        return icon;
    }

    let ext = lower.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    by_extension(ext)
}

pub fn folder(expanded: bool) -> (&'static str, Color) {
    if expanded {
        (FOLDER_OPEN, FOLDER_COLOR)
    } else {
        (FOLDER_CLOSED, FOLDER_COLOR)
    }
}

/// The disclosure arrow nvim-tree draws to the left of a directory's icon.
pub fn arrow(expanded: bool) -> &'static str {
    if expanded {
        "▾"
    } else {
        "▸"
    }
}

fn special_cased(lower: &str) -> Option<(&'static str, Color)> {
    Some(match lower {
        "cargo.toml" => (COG, Color::Rgb(222, 165, 132)),
        "cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => {
            (LOCK, Color::DarkGray)
        }
        "dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => {
            (FILE_CODE, Color::Rgb(56, 150, 214))
        }
        ".gitignore" | ".gitmodules" | ".gitattributes" => (GIT, Color::Rgb(240, 80, 50)),
        "package.json" => (COG, Color::Rgb(203, 203, 65)),
        "readme.md" | "readme" | "readme.txt" => (FILE_TEXT, Color::Rgb(148, 163, 184)),
        "license" | "license.md" | "license.txt" => (FILE_TEXT, Color::Rgb(203, 203, 65)),
        _ if lower.starts_with(".env") => (COG, Color::Rgb(197, 195, 71)),
        _ => return None,
    })
}

fn by_extension(ext: &str) -> (&'static str, Color) {
    match ext {
        "rs" => (FILE_CODE, Color::Rgb(222, 165, 132)),
        "js" | "mjs" | "cjs" | "jsx" => (FILE_CODE, Color::Rgb(203, 203, 65)),
        "ts" | "tsx" => (FILE_CODE, Color::Rgb(49, 120, 198)),
        "json" | "jsonc" => (COG, Color::Rgb(203, 203, 65)),
        "md" | "markdown" => (FILE_TEXT, Color::Rgb(148, 163, 184)),
        "toml" => (COG, Color::Rgb(156, 66, 33)),
        "yml" | "yaml" => (COG, Color::Rgb(203, 23, 30)),
        "py" => (FILE_CODE, Color::Rgb(255, 213, 79)),
        "html" | "htm" => (FILE_CODE, Color::Rgb(228, 77, 38)),
        "css" => (FILE_CODE, Color::Rgb(38, 77, 228)),
        "scss" | "sass" => (FILE_CODE, Color::Rgb(205, 105, 149)),
        "sh" | "bash" | "zsh" | "fish" => (TERMINAL, Color::Rgb(137, 224, 81)),
        "lock" => (LOCK, Color::DarkGray),
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" | "bmp" => {
            (FILE_IMAGE, Color::Rgb(212, 106, 135))
        }
        "pdf" => (FILE_PDF, Color::Red),
        "zip" | "tar" | "gz" | "xz" | "7z" | "rar" => (FILE_ARCHIVE, Color::Rgb(214, 196, 161)),
        "sql" | "db" | "sqlite" => (DATABASE, Color::Rgb(105, 179, 76)),
        "lua" => (FILE_CODE, Color::Rgb(0, 0, 255)),
        "c" => (FILE_CODE, Color::Rgb(85, 155, 213)),
        "h" | "hpp" => (FILE_CODE, Color::Rgb(163, 105, 217)),
        "cpp" | "cc" | "cxx" => (FILE_CODE, Color::Rgb(243, 75, 125)),
        "go" => (FILE_CODE, Color::Rgb(0, 173, 216)),
        "txt" => (FILE_TEXT, Color::Rgb(203, 203, 203)),
        _ => (FILE_GENERIC, Color::Rgb(160, 160, 160)),
    }
}
