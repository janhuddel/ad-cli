//! Inline fuzzy-filter list picker.
//!
//! Replaces `dialoguer::FuzzySelect` for two reasons: dialoguer clears and
//! redraws line by line (one Win32 console call per line on Windows), which
//! flickers badly on long lists, and `console` — which dialoguer reads keys
//! through — does not map PageUp/PageDown on Windows at all. Here every frame
//! is queued and written in a single flush, and keys come from `crossterm`.

use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    queue,
    style::{Attribute, Print, SetAttribute, Stylize},
    terminal::{self, Clear, ClearType},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::error::Result;

/// Rows shown at once; capped further by the terminal height.
const MAX_VISIBLE_ROWS: usize = 15;

/// Lets the user filter `items` by typing and pick one with the arrow keys,
/// PageUp/PageDown, Home/End and Enter. Returns the index into `items`, or
/// `None` if aborted with Esc / Ctrl+C. Renders to stderr, leaving no trace
/// once it returns.
pub fn pick(prompt: &str, items: &[String]) -> Result<Option<usize>> {
    let mut out = io::stderr();
    let _guard = RawModeGuard::enable(&mut out)?;

    let (_, term_rows) = terminal::size()?;
    // Prompt line + list + footer line, and keep one spare row so drawing the
    // last line never scrolls the frame out from under the cursor math.
    let rows = MAX_VISIBLE_ROWS
        .min((term_rows as usize).saturating_sub(3))
        .max(1);
    let mut state = State::new(items, rows);
    let mut drawn = false;

    let result = loop {
        draw(&mut out, prompt, &state, drawn)?;
        drawn = true;

        let key = match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => key,
            _ => continue, // resize etc.: just redraw
        };
        match state.handle_key(key) {
            Outcome::Continue => {}
            Outcome::Picked(index) => break Some(index),
            Outcome::Aborted => break None,
        }
    };

    erase(&mut out, state.frame_height())?;
    Ok(result)
}

enum Outcome {
    Continue,
    Picked(usize),
    Aborted,
}

struct State<'a> {
    items: &'a [String],
    matcher: SkimMatcherV2,
    query: String,
    /// Indices into `items`, best match first.
    filtered: Vec<usize>,
    /// Position within `filtered`.
    selected: usize,
    /// First visible position within `filtered`.
    offset: usize,
    rows: usize,
}

impl<'a> State<'a> {
    fn new(items: &'a [String], rows: usize) -> Self {
        let mut state = State {
            items,
            matcher: SkimMatcherV2::default(),
            query: String::new(),
            filtered: Vec::new(),
            selected: 0,
            offset: 0,
            rows,
        };
        state.refilter();
        state
    }

    fn frame_height(&self) -> usize {
        self.rows + 2
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.items.len()).collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| self.matcher.fuzzy_match(item, &self.query).map(|s| (i, s)))
                .collect();
            // Stable sort keeps the original order among equal scores.
            scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        self.selected = 0;
        self.offset = 0;
    }

    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        let len = self.filtered.len();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return Outcome::Aborted,
            KeyCode::Char('c') if ctrl => return Outcome::Aborted,
            KeyCode::Enter if len > 0 => return Outcome::Picked(self.filtered[self.selected]),
            KeyCode::Up | KeyCode::BackTab if len > 0 => {
                self.selected = (self.selected + len - 1) % len;
            }
            KeyCode::Down | KeyCode::Tab if len > 0 => {
                self.selected = (self.selected + 1) % len;
            }
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(self.rows),
            KeyCode::PageDown if len > 0 => {
                self.selected = (self.selected + self.rows).min(len - 1);
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = len.saturating_sub(1),
            KeyCode::Backspace => {
                if self.query.pop().is_some() {
                    self.refilter();
                }
            }
            KeyCode::Char(c) if !ctrl && !c.is_control() => {
                self.query.push(c);
                self.refilter();
            }
            _ => {}
        }
        self.scroll_to_selection();
        Outcome::Continue
    }

    fn scroll_to_selection(&mut self) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + self.rows {
            self.offset = self.selected + 1 - self.rows;
        }
    }
}

/// Draws one complete frame with a single flush. After the first frame the
/// cursor sits on the frame's last line, so redrawing starts by moving back
/// up to its first line and overwriting in place — nothing is cleared first,
/// which is what avoids the flicker.
fn draw(out: &mut impl Write, prompt: &str, state: &State, redraw: bool) -> io::Result<()> {
    let (cols, _) = terminal::size()?;
    // Never fill the last column: some terminals wrap there, which would
    // shift every following line by one.
    let width = (cols as usize).saturating_sub(1);

    if redraw {
        queue!(
            out,
            cursor::MoveToColumn(0),
            cursor::MoveUp(state.frame_height() as u16 - 1)
        )?;
    }

    let header = format!("? {prompt} › ");
    queue!(out, Print(truncate(&header, width).bold()))?;
    let remaining = width.saturating_sub(header.chars().count());
    queue!(
        out,
        Print(truncate(&state.query, remaining)),
        Clear(ClearType::UntilNewLine)
    )?;

    for row in 0..state.rows {
        queue!(out, Print("\r\n"))?;
        if let Some(&index) = state.filtered.get(state.offset + row) {
            let is_selected = state.offset + row == state.selected;
            draw_item(out, state, &state.items[index], is_selected, width)?;
        }
        queue!(out, Clear(ClearType::UntilNewLine))?;
    }

    let footer = if state.filtered.is_empty() {
        "  no matches".to_string()
    } else {
        format!(
            "  {}/{}  ·  ↑↓ PgUp/PgDn Home/End",
            state.selected + 1,
            state.filtered.len()
        )
    };
    queue!(
        out,
        Print("\r\n"),
        Print(truncate(&footer, width).dark_grey()),
        Clear(ClearType::UntilNewLine)
    )?;
    out.flush()
}

fn draw_item(
    out: &mut impl Write,
    state: &State,
    item: &str,
    is_selected: bool,
    width: usize,
) -> io::Result<()> {
    if is_selected {
        queue!(out, Print("> ".cyan().bold()))?;
    } else {
        queue!(out, Print("  "))?;
    }

    let matched = if state.query.is_empty() {
        Vec::new()
    } else {
        state
            .matcher
            .fuzzy_indices(item, &state.query)
            .map(|(_, indices)| indices)
            .unwrap_or_default()
    };

    let max_chars = width.saturating_sub(2);
    let truncated = item.chars().count() > max_chars;
    let keep = if truncated {
        max_chars.saturating_sub(1)
    } else {
        max_chars
    };
    for (i, c) in item.chars().take(keep).enumerate() {
        if matched.binary_search(&i).is_ok() {
            queue!(out, Print(c.yellow().bold()))?;
        } else if is_selected {
            queue!(out, Print(c.cyan()))?;
        } else {
            queue!(out, Print(c))?;
        }
    }
    if truncated {
        queue!(out, Print('…'))?;
    }
    queue!(out, SetAttribute(Attribute::Reset))
}

fn erase(out: &mut impl Write, frame_height: usize) -> io::Result<()> {
    queue!(
        out,
        cursor::MoveToColumn(0),
        cursor::MoveUp(frame_height as u16 - 1),
        Clear(ClearType::FromCursorDown)
    )?;
    out.flush()
}

/// Truncates to at most `max` characters, marking the cut with `…`.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Restores cooked mode and the cursor even on early return or panic.
struct RawModeGuard;

impl RawModeGuard {
    fn enable(out: &mut impl Write) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        queue!(out, cursor::Hide)?;
        out.flush()?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut out = io::stderr();
        let _ = queue!(out, cursor::Show);
        let _ = out.flush();
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn items(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("group-{i:03}")).collect()
    }

    #[test]
    fn page_keys_move_by_visible_rows_and_clamp() {
        let items = items(40);
        let mut s = State::new(&items, 15);
        s.handle_key(key(KeyCode::PageDown));
        assert_eq!((s.selected, s.offset), (15, 1));
        s.handle_key(key(KeyCode::PageDown));
        s.handle_key(key(KeyCode::PageDown));
        assert_eq!((s.selected, s.offset), (39, 25));
        s.handle_key(key(KeyCode::PageUp));
        assert_eq!((s.selected, s.offset), (24, 24));
        s.handle_key(key(KeyCode::Home));
        assert_eq!((s.selected, s.offset), (0, 0));
        s.handle_key(key(KeyCode::End));
        assert_eq!((s.selected, s.offset), (39, 25));
    }

    #[test]
    fn arrows_wrap_around() {
        let items = items(5);
        let mut s = State::new(&items, 3);
        s.handle_key(key(KeyCode::Up));
        assert_eq!((s.selected, s.offset), (4, 2));
        s.handle_key(key(KeyCode::Down));
        assert_eq!((s.selected, s.offset), (0, 0));
    }

    #[test]
    fn typing_filters_and_enter_returns_original_index() {
        let items = vec![
            "GG-Finance".into(),
            "GG-IT-Admins".into(),
            "GG-IT-Users".into(),
        ];
        let mut s = State::new(&items, 15);
        for c in "users".chars() {
            s.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(s.filtered, vec![2]);
        assert!(matches!(
            s.handle_key(key(KeyCode::Enter)),
            Outcome::Picked(2)
        ));
    }

    #[test]
    fn duplicate_labels_keep_distinct_indices() {
        let items = vec!["Same".into(), "Same".into()];
        let mut s = State::new(&items, 15);
        s.handle_key(key(KeyCode::Down));
        assert!(matches!(
            s.handle_key(key(KeyCode::Enter)),
            Outcome::Picked(1)
        ));
    }
}
