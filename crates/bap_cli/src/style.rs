//! Terminal styling gated on TTY state and `NO_COLOR`.
//!
//! Every helper returns the input unchanged when color is disabled, so call
//! sites stay unconditional.

use std::io::IsTerminal;
use std::sync::OnceLock;

/// Whether ANSI colors may be emitted on stdout/stderr.
pub fn colors_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let term_dumb =
            std::env::var_os("TERM").is_some_and(|term| term.eq_ignore_ascii_case("dumb"));
        let tty = std::io::stdout().is_terminal() && std::io::stderr().is_terminal();
        colors_enabled_with(no_color, term_dumb, tty)
    })
}

/// Pure decision core: honor `NO_COLOR` and `TERM=dumb`, else require a TTY.
fn colors_enabled_with(no_color: bool, term_dumb: bool, tty: bool) -> bool {
    !no_color && !term_dumb && tty
}

fn wrap(code: &str, text: &str) -> String {
    if colors_enabled() {
        format!("\u{1b}[{code}m{text}\u{1b}[0m")
    } else {
        text.to_owned()
    }
}

/// Green text for success states.
pub fn good(text: &str) -> String {
    wrap("32", text)
}

/// Red text for failures.
pub fn bad(text: &str) -> String {
    wrap("31", text)
}

/// Yellow text for warnings and gaps.
pub fn warn(text: &str) -> String {
    wrap("33", text)
}

/// Cyan text for prompts and highlights.
pub fn accent(text: &str) -> String {
    wrap("36", text)
}

/// Dimmed secondary text.
pub fn dim(text: &str) -> String {
    wrap("2", text)
}

/// Bold text for headings.
pub fn bold(text: &str) -> String {
    wrap("1", text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_and_dumb_term_disable_color_even_on_a_tty() {
        assert!(!colors_enabled_with(true, false, true));
        assert!(!colors_enabled_with(false, true, true));
    }

    #[test]
    fn color_requires_a_tty() {
        assert!(!colors_enabled_with(false, false, false));
        assert!(colors_enabled_with(false, false, true));
    }

    #[test]
    fn styled_output_preserves_content() {
        for text in ["", "plain", "with | pipes | inside"] {
            assert!(accent(text).contains(text));
            assert!(bad(text).contains(text));
        }
    }
}
