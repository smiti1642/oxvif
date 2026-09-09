// Backend-free Vim-style navigation. MIT, same license as oxvif.
// This file can also be compiled directly with `rustc --test`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Up,
    Down,
    PageUp,
    PageDown,
    HalfUp,
    HalfDown,
    Home,
    End,
    Escape,
    Interrupt,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Up(usize),
    Down(usize),
    PageUp(usize),
    PageDown(usize),
    HalfUp(usize),
    HalfDown(usize),
    First,
    Last,
    At(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Action(Motion),
    Consumed,
    Unhandled,
}

#[derive(Clone, Debug, Default)]
pub struct Navigation {
    count: Option<usize>,
    waiting_g: bool,
    hint: &'static str,
}

impl Navigation {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn pending(&self) -> String {
        format!(
            "{}{}",
            self.count.map(|n| n.to_string()).unwrap_or_default(),
            if self.waiting_g { "g" } else { "" }
        )
    }
    pub fn hint(&self) -> &'static str {
        self.hint
    }

    pub fn feed(&mut self, key: Key) -> Outcome {
        self.hint = "";
        let has_prefix = self.count.is_some() || self.waiting_g;
        if key == Key::Interrupt {
            self.reset();
            return Outcome::Unhandled;
        }
        if key == Key::Escape {
            self.reset();
            return if has_prefix {
                Outcome::Consumed
            } else {
                Outcome::Unhandled
            };
        }
        if self.waiting_g {
            if key == Key::Char('g') {
                let motion = self.count.map_or(Motion::First, Motion::At);
                self.reset();
                return Outcome::Action(motion);
            }
            return self.invalid();
        }
        if let Key::Char(c @ '0'..='9') = key {
            let digit = (c as u8 - b'0') as usize;
            if self.count.is_none() && digit == 0 {
                self.hint = "Count starts at 1";
                return Outcome::Consumed;
            }
            let next = self
                .count
                .unwrap_or(0)
                .saturating_mul(10)
                .saturating_add(digit);
            if next > 999_999 {
                self.hint = "Count limited to 999999";
            } else {
                self.count = Some(next);
            }
            return Outcome::Pending;
        }
        let count = self.count.unwrap_or(1);
        let motion = match key {
            Key::Char('g') => {
                self.waiting_g = true;
                return Outcome::Pending;
            }
            Key::Char('G') => self.count.map_or(Motion::Last, Motion::At),
            Key::Char('j') | Key::Down => Motion::Down(count),
            Key::Char('k') | Key::Up => Motion::Up(count),
            Key::PageDown => Motion::PageDown(count),
            Key::PageUp => Motion::PageUp(count),
            Key::HalfDown => Motion::HalfDown(count),
            Key::HalfUp => Motion::HalfUp(count),
            Key::Home if !has_prefix => Motion::First,
            Key::End if !has_prefix => Motion::Last,
            _ if has_prefix => return self.invalid(),
            _ => return Outcome::Unhandled,
        };
        self.reset();
        Outcome::Action(motion)
    }
    fn invalid(&mut self) -> Outcome {
        self.reset();
        self.hint = "Unsupported sequence; cancelled";
        Outcome::Consumed
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub selected: usize,
    pub top: usize,
}

impl Viewport {
    pub fn clamp(&mut self, count: usize, rows: usize) {
        let rows = rows.max(1);
        self.selected = self.selected.min(count.saturating_sub(1));
        self.top = self.top.min(count.saturating_sub(rows)).min(self.selected);
        if self.selected.saturating_sub(self.top) >= rows {
            self.top = self.selected.saturating_sub(rows - 1);
        }
    }
    pub fn apply(&mut self, motion: Motion, count: usize, rows: usize) {
        self.clamp(count, rows);
        let rows = rows.max(1);
        self.selected = destination(self.selected, motion, count.saturating_sub(1), rows);
        match motion {
            Motion::PageUp(_) | Motion::PageDown(_) | Motion::HalfUp(_) | Motion::HalfDown(_) => {
                self.top = destination(self.top, motion, count.saturating_sub(rows), rows);
            }
            Motion::First => self.top = 0,
            Motion::Last => self.top = count.saturating_sub(rows),
            _ => {}
        }
        self.clamp(count, rows);
    }
}

/// A text viewer anchors navigation at its first visible wrapped display line.
pub fn scroll(top: usize, motion: Motion, count: usize, rows: usize) -> usize {
    destination(top, motion, count.saturating_sub(rows.max(1)), rows.max(1))
}

fn destination(position: usize, motion: Motion, maximum: usize, rows: usize) -> usize {
    let half = (rows / 2).max(1);
    match motion {
        Motion::Up(n) => position.saturating_sub(n),
        Motion::Down(n) => position.saturating_add(n),
        Motion::PageUp(n) => position.saturating_sub(rows.saturating_mul(n)),
        Motion::PageDown(n) => position.saturating_add(rows.saturating_mul(n)),
        Motion::HalfUp(n) => position.saturating_sub(half.saturating_mul(n)),
        Motion::HalfDown(n) => position.saturating_add(half.saturating_mul(n)),
        Motion::First => 0,
        Motion::Last => maximum,
        Motion::At(n) => n.saturating_sub(1),
    }
    .min(maximum)
}

pub fn relative_number(index: usize, selected: usize) -> usize {
    if index == selected {
        index.saturating_add(1)
    } else {
        index.abs_diff(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sequence(text: &str) -> (Navigation, Outcome) {
        let mut nav = Navigation::default();
        let mut result = Outcome::Unhandled;
        for c in text.chars() {
            result = nav.feed(Key::Char(c));
        }
        (nav, result)
    }
    #[test]
    fn grammar_and_counts() {
        for (text, expected) in [
            ("gg", Motion::First),
            ("G", Motion::Last),
            ("12j", Motion::Down(12)),
            ("5k", Motion::Up(5)),
            ("42G", Motion::At(42)),
            ("42gg", Motion::At(42)),
            ("10j", Motion::Down(10)),
        ] {
            let (nav, result) = sequence(text);
            assert_eq!(result, Outcome::Action(expected), "{text}");
            assert_eq!(nav.pending(), "");
        }
        let (nav, result) = sequence("12g");
        assert_eq!(result, Outcome::Pending);
        assert_eq!(nav.pending(), "12g");
    }
    #[test]
    fn cancellation_limits_and_invalid_actions() {
        for text in ["3i", "gq", "gG", "12g1"] {
            let (nav, result) = sequence(text);
            assert_eq!(result, Outcome::Consumed, "{text}");
            assert_eq!(nav.pending(), "");
            assert_eq!(nav.hint(), "Unsupported sequence; cancelled");
        }
        let (mut nav, _) = sequence("9999999");
        assert_eq!(nav.pending(), "999999");
        assert_eq!(nav.hint(), "Count limited to 999999");
        assert_eq!(nav.feed(Key::Escape), Outcome::Consumed);
        assert_eq!(nav.feed(Key::Escape), Outcome::Unhandled);
        nav.feed(Key::Char('g'));
        assert_eq!(nav.feed(Key::Interrupt), Outcome::Unhandled);
        assert_eq!(nav.pending(), "");
        assert_eq!(nav.feed(Key::Char('0')), Outcome::Consumed);
        assert_eq!(nav.hint(), "Count starts at 1");
        assert_eq!(nav.feed(Key::Other), Outcome::Unhandled);
    }
    #[test]
    fn backend_neutral_special_keys() {
        for (key, motion) in [
            (Key::Up, Motion::Up(3)),
            (Key::Down, Motion::Down(3)),
            (Key::HalfUp, Motion::HalfUp(3)),
            (Key::HalfDown, Motion::HalfDown(3)),
            (Key::PageUp, Motion::PageUp(3)),
            (Key::PageDown, Motion::PageDown(3)),
        ] {
            let (mut nav, _) = sequence("3");
            assert_eq!(nav.feed(key), Outcome::Action(motion));
        }
        let mut nav = Navigation::default();
        assert_eq!(nav.feed(Key::Home), Outcome::Action(Motion::First));
        assert_eq!(nav.feed(Key::End), Outcome::Action(Motion::Last));
        nav.feed(Key::Char('2'));
        assert_eq!(nav.feed(Key::Home), Outcome::Consumed);
        nav.reset();
        assert_eq!(nav.hint(), "");
    }
    #[test]
    fn bounded_viewport_and_relative_numbers() {
        let mut view = Viewport::default();
        view.apply(Motion::Down(7), 40, 5);
        assert_eq!(
            view,
            Viewport {
                selected: 7,
                top: 3
            }
        );
        view.apply(Motion::HalfDown(1), 40, 5);
        assert_eq!(
            view,
            Viewport {
                selected: 9,
                top: 5
            }
        );
        view.apply(Motion::Last, 40, 5);
        assert_eq!(
            view,
            Viewport {
                selected: 39,
                top: 35
            }
        );
        view.apply(Motion::At(21), 40, 5);
        assert_eq!(
            view,
            Viewport {
                selected: 20,
                top: 20
            }
        );
        assert_eq!(relative_number(20, 20), 21);
        assert_eq!(relative_number(13, 20), 7);
        assert_eq!(relative_number(27, 20), 7);
        assert_eq!(scroll(0, Motion::At(21), 40, 5), 20);
        assert_eq!(scroll(0, Motion::Last, 40, 5), 35);
        for count in [0, 1, 40, usize::MAX] {
            for rows in [0, 1, 5, 80] {
                for motion in [
                    Motion::First,
                    Motion::Last,
                    Motion::Down(usize::MAX),
                    Motion::PageDown(usize::MAX),
                    Motion::Up(usize::MAX),
                    Motion::HalfUp(usize::MAX),
                ] {
                    view.apply(motion, count, rows);
                    assert!(view.selected <= count.saturating_sub(1));
                    assert!(view.top <= view.selected);
                    assert!(view.selected - view.top < rows.max(1));
                    assert!(
                        scroll(usize::MAX, motion, count, rows)
                            <= count.saturating_sub(rows.max(1))
                    );
                }
            }
        }
    }
}
