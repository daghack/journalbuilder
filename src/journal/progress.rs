//! Progress on the command line: one line of the terminal, redrawn in
//! place, with a bar, a count, what's being worked on and the time left.
//!
//! It's drawn on stderr only when that's a terminal, so logs and tests
//! stay clean; `finish` clears it.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

/// The bar's width, in characters.
const BAR: usize = 24;
/// The whole line's width, at most, unless `COLUMNS` says otherwise.
const LINE: usize = 100;
/// A spinner's frames, and how often a spinning line is redrawn.
const SPIN: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPIN_EVERY: Duration = Duration::from_millis(80);

pub struct Progress {
    what: String,
    total: usize,
    done: usize,
    item: String,
    started: Instant,
    last_drawn: Option<Instant>,
    frame: usize,
    live: bool,
}

impl Progress {
    /// A bar for `total` steps of `what` ("Rendering"), drawn if stderr is
    /// a terminal. A total of 0 is a spinner, for work of unknown length.
    pub fn new(what: &str, total: usize) -> Self {
        Progress {
            what: what.to_string(),
            total,
            done: 0,
            item: String::new(),
            started: Instant::now(),
            last_drawn: None,
            frame: 0,
            live: std::io::stderr().is_terminal() && std::env::var_os("NO_PROGRESS").is_none(),
        }
    }

    /// Working on `item` now.
    pub fn start(&mut self, item: &str) {
        self.item = item.to_string();
        self.draw();
    }

    /// One step done.
    pub fn step(&mut self) {
        self.done += 1;
        self.draw();
    }

    /// Redraw a spinner, if it's due: for work that reports in often.
    pub fn tick(&mut self, item: &str) {
        if self.last_drawn.is_some_and(|t| t.elapsed() < SPIN_EVERY) {
            return;
        }
        self.frame += 1;
        self.start(item);
    }

    /// How long since it started.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Clear the line, leaving the terminal as it was.
    pub fn finish(&mut self) {
        if self.live && self.last_drawn.is_some() {
            eprint!("\r\x1b[2K");
            let _ = std::io::stderr().flush();
        }
        self.last_drawn = None;
    }

    fn draw(&mut self) {
        if !self.live {
            return;
        }
        self.last_drawn = Some(Instant::now());
        let width = std::env::var("COLUMNS").ok().and_then(|c| c.parse::<usize>().ok()).unwrap_or(LINE).min(LINE);
        let line = self.line(width);
        eprint!("\r\x1b[2K{}", line);
        let _ = std::io::stderr().flush();
    }

    /// The line as drawn, at most `width` characters.
    fn line(&self, width: usize) -> String {
        let elapsed = self.started.elapsed();
        let head = if self.total == 0 {
            format!("{} {} {}", SPIN[self.frame % SPIN.len()], self.what, clock(elapsed))
        } else {
            let fraction = self.done as f64 / self.total as f64;
            let filled = (fraction * BAR as f64).round() as usize;
            let bar: String = "█".repeat(filled) + "░".repeat(BAR - filled.min(BAR)).as_str();
            let left = if self.done > 0 && self.done < self.total {
                let per = elapsed.as_secs_f64() / self.done as f64;
                format!(", ~{} left", clock(Duration::from_secs_f64(per * (self.total - self.done) as f64)))
            } else {
                String::new()
            };
            format!("{} {} {:>3}/{} {}{}", self.what, bar, self.done, self.total, clock(elapsed), left)
        };
        let room = width.saturating_sub(head.chars().count() + 2);
        if self.item.is_empty() || room < 4 {
            return head;
        }
        let item: String = if self.item.chars().count() > room {
            self.item.chars().take(room - 1).collect::<String>() + "…"
        } else {
            self.item.clone()
        };
        format!("{}  {}", head, item)
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.finish();
    }
}

/// `m:ss`.
pub fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_show_the_bar_count_and_item_within_their_width() {
        let mut p = Progress::new("Rendering", 4);
        p.done = 2;
        p.item = "a_very_long_page_name_that_goes_on_and_on".to_string();
        let line = p.line(80);
        assert!(line.starts_with("Rendering ████████████░░░░░░░░░░░░   2/4 0:00"), "{}", line);
        assert!(line.chars().count() <= 80 && line.ends_with('…'), "{}", line);
        let spinner = Progress::new("Running", 0).line(60);
        assert!(spinner.contains("Running 0:00"), "{}", spinner);
    }
}
