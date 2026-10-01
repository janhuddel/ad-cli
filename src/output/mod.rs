pub mod groups_view;
pub mod user_view;

use std::io::IsTerminal;

pub fn stdout_is_interactive() -> bool {
    std::io::stdout().is_terminal()
}
