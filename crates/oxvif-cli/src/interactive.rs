use std::{
    future::Future,
    io::{self, Write},
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Print, SetAttribute},
    terminal::{
        self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate,
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use oxvif_cli::{
    AppError, DiscoveryDeviceView, DiscoveryRecord, DiscoveryRegistrationStatus,
    DiscoveryResultSummary, SecretString, discovery_query_matches, normalize_target,
};
use tokio::time::{Instant, MissedTickBehavior, interval};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zeroize::Zeroize;

#[cfg(unix)]
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};

use crate::navigation::{self, Key as NavKey, Navigation, Outcome, Viewport};

use crate::ui_settings::{self, LineNumbers};

const DEFAULT_PAGE_SIZE: usize = 12;

pub(crate) enum BrowserAction {
    Quit,
    Add(Box<DiscoverySetup>),
}

pub(crate) struct DiscoverySetup {
    pub(crate) device: DiscoveryRecord,
    pub(crate) id: String,
    pub(crate) username: String,
    pub(crate) password: SecretString,
}

enum BrowserIntent {
    Quit,
    Settings,
    Activate(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiscoveryPurpose {
    Add,
    Select,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum RegistrationView {
    #[default]
    All,
    Saved,
    Unregistered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SetupField {
    Id,
    Username,
    Password,
}

enum SetupIntent {
    Cancel,
    Quit,
    Submit,
}

struct SetupForm {
    device: DiscoveryRecord,
    id: String,
    username: String,
    password: String,
    field: SetupField,
    error: Option<String>,
}

impl SetupForm {
    fn new(device: DiscoveryRecord, suggested_id: String) -> Self {
        Self {
            device,
            id: suggested_id,
            username: String::new(),
            password: String::new(),
            field: SetupField::Id,
            error: None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<SetupIntent> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(SetupIntent::Quit);
        }
        match key.code {
            KeyCode::Esc => return Some(SetupIntent::Cancel),
            KeyCode::Tab | KeyCode::Down => self.next_field(),
            KeyCode::BackTab | KeyCode::Up => self.previous_field(),
            KeyCode::Enter => {
                if matches!(self.field, SetupField::Password) {
                    if self.validate() {
                        return Some(SetupIntent::Submit);
                    }
                } else {
                    self.next_field();
                }
            }
            KeyCode::Backspace => {
                self.current_value_mut().pop();
                self.error = None;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.current_value_mut().clear();
                self.error = None;
            }
            KeyCode::Char(character)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.current_value_mut().push(character);
                self.error = None;
            }
            _ => {}
        }
        None
    }

    fn paste(&mut self, value: &str) {
        self.current_value_mut().push_str(value);
        self.error = None;
    }

    fn current_value_mut(&mut self) -> &mut String {
        match self.field {
            SetupField::Id => &mut self.id,
            SetupField::Username => &mut self.username,
            SetupField::Password => &mut self.password,
        }
    }

    fn next_field(&mut self) {
        self.field = match self.field {
            SetupField::Id => SetupField::Username,
            SetupField::Username | SetupField::Password => SetupField::Password,
        };
        self.error = None;
    }

    fn previous_field(&mut self) {
        self.field = match self.field {
            SetupField::Id | SetupField::Username => SetupField::Id,
            SetupField::Password => SetupField::Username,
        };
        self.error = None;
    }

    fn validate(&mut self) -> bool {
        let missing = if self.id.trim().is_empty() {
            Some((SetupField::Id, "Device ID must not be empty."))
        } else if self.username.trim().is_empty() {
            Some((SetupField::Username, "Username must not be empty."))
        } else if self.password.is_empty() {
            Some((SetupField::Password, "Password must not be empty."))
        } else {
            None
        };
        if let Some((field, message)) = missing {
            self.field = field;
            self.error = Some(message.to_owned());
            false
        } else {
            true
        }
    }

    fn finish(mut self) -> Result<DiscoverySetup, AppError> {
        Ok(DiscoverySetup {
            device: self.device.clone(),
            id: self.id.trim().to_owned(),
            username: self.username.trim().to_owned(),
            password: SecretString::new(std::mem::take(&mut self.password))?,
        })
    }
}

impl Drop for SetupForm {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

/// Delayed, single-line progress; the caller must enforce interactive table output.
pub(crate) async fn await_workflow<F: Future>(future: F, label: &str) -> F::Output {
    let started = Instant::now();
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;
    let mut progress = WorkflowProgress(false);
    tokio::pin!(future);
    loop {
        tokio::select! {
            output = &mut future => return output,
            _ = ticker.tick() => {
                progress.0 = true;
                let width = terminal::size().map(|s| s.0.saturating_sub(1) as usize).unwrap_or(79);
                let text = truncate_to_width(&format!("{label}... {}s elapsed", started.elapsed().as_secs()), width);
                let _ = execute!(io::stderr(), MoveToColumn(0), Clear(ClearType::CurrentLine), Print(text));
            }
        }
    }
}

struct WorkflowProgress(bool);
impl Drop for WorkflowProgress {
    fn drop(&mut self) {
        if self.0 {
            clear_workflow_progress();
        }
    }
}

pub(crate) fn clear_workflow_progress() {
    let _ = execute!(io::stderr(), MoveToColumn(0), Clear(ClearType::CurrentLine));
}

/// Bounded profile selector using the same terminal lifecycle as discovery.
pub(crate) fn select_profile(choices: &[String]) -> Result<Option<usize>, AppError> {
    if choices.is_empty() {
        return Err(AppError::invalid_argument("No profiles are available."));
    }
    Panel::enter()?.menu("Select media profile", choices, &[])
}

fn navigation_key(nav: &mut Navigation, key: KeyEvent, discovery: bool) -> Outcome {
    if key.kind == KeyEventKind::Release
        || (key.kind == KeyEventKind::Repeat && matches!(key.code, KeyCode::Char('0'..='9' | 'g')))
    {
        return Outcome::Consumed;
    }
    let plain = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
    let mapped = if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('c') => NavKey::Interrupt,
            KeyCode::Char('d') => NavKey::HalfDown,
            KeyCode::Char('u') => NavKey::HalfUp,
            _ => NavKey::Other,
        }
    } else if plain {
        match key.code {
            KeyCode::Char('h' | 'l') | KeyCode::Left | KeyCode::Right if discovery => {
                if nav.pending().is_empty() {
                    if matches!(key.code, KeyCode::Char('h') | KeyCode::Left) {
                        NavKey::PageUp
                    } else {
                        NavKey::PageDown
                    }
                } else {
                    NavKey::Other
                }
            }
            KeyCode::Char(c) => NavKey::Char(c),
            KeyCode::Up => NavKey::Up,
            KeyCode::Down => NavKey::Down,
            KeyCode::PageUp => NavKey::PageUp,
            KeyCode::PageDown => NavKey::PageDown,
            KeyCode::Home => NavKey::Home,
            KeyCode::End => NavKey::End,
            KeyCode::Esc => NavKey::Escape,
            _ => NavKey::Other,
        }
    } else {
        NavKey::Other
    };
    let result = nav.feed(mapped);
    if result == Outcome::Unhandled && !plain && mapped != NavKey::Interrupt {
        Outcome::Consumed
    } else {
        result
    }
}

fn nav_status(
    mode: &str,
    nav: &Navigation,
    index: usize,
    count: usize,
    numbers: LineNumbers,
) -> String {
    let pending = nav.pending();
    format!(
        "{}{} | {}{}/{} | numbers:{}{}",
        if mode.contains("TEXT") {
            "NORMAL"
        } else {
            mode
        },
        if pending.is_empty() {
            String::new()
        } else {
            format!(" | [{pending}]")
        },
        if mode.contains("TEXT") {
            "line "
        } else {
            "item "
        },
        if count == 0 {
            0
        } else {
            index.saturating_add(1)
        },
        count,
        numbers.name(),
        if nav.hint().is_empty() {
            String::new()
        } else {
            format!(" | {}", nav.hint())
        }
    )
}

fn gutter_width(count: usize, width: usize, mode: LineNumbers) -> usize {
    if mode == LineNumbers::Off {
        return 2.min(width);
    }
    let gutter = count.max(1).to_string().len() + 3;
    if width >= gutter + 12 {
        gutter
    } else {
        2.min(width)
    }
}

fn numbered_line(
    text: &str,
    index: usize,
    selected: usize,
    count: usize,
    width: usize,
    mode: LineNumbers,
) -> String {
    let marker = if index == selected { '>' } else { ' ' };
    let gutter = gutter_width(count, width, mode);
    if gutter > 2 {
        format!(
            "{marker} {:>digits$} {text}",
            mode.number(index, selected).unwrap_or(0),
            digits = gutter - 3
        )
    } else {
        format!("{marker} {text}")
    }
}

fn menu_frame(
    title: &str,
    choices: &[String],
    view: Viewport,
    nav: &Navigation,
    width: u16,
    height: u16,
    mode: LineNumbers,
) -> Vec<String> {
    let body = choices
        .iter()
        .enumerate()
        .skip(view.top)
        .take(panel_body_rows(height))
        .map(|(i, s)| {
            numbered_line(
                s,
                i,
                view.selected,
                choices.len(),
                width.saturating_sub(1) as usize,
                mode,
            )
        })
        .collect::<Vec<_>>();
    panel_lines(
        title,
        &body,
        "? settings | j/k 7j/3k gg/G nG | PgUp/Dn ^D/^U | Enter select | i info | q back",
        &nav_status("NORMAL", nav, view.selected, choices.len(), mode),
        width,
        height,
    )
}

pub(crate) async fn await_discovery<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    let mut progress = DiscoveryProgress::start();
    let started = Instant::now();
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;
    tokio::pin!(future);

    loop {
        tokio::select! {
            output = &mut future => return output,
            _ = ticker.tick() => progress.update(started.elapsed()),
        }
    }
}

struct DiscoveryProgress {
    stderr: io::Stderr,
}

impl DiscoveryProgress {
    fn start() -> Self {
        let mut progress = Self {
            stderr: io::stderr(),
        };
        progress.update(Duration::ZERO);
        progress
    }

    fn update(&mut self, elapsed: Duration) {
        let _ = queue!(
            self.stderr,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print(discovery_progress_text(elapsed))
        );
        let _ = self.stderr.flush();
    }
}

impl Drop for DiscoveryProgress {
    fn drop(&mut self) {
        let _ = queue!(self.stderr, MoveToColumn(0), Clear(ClearType::CurrentLine));
        let _ = self.stderr.flush();
    }
}

fn discovery_progress_text(elapsed: Duration) -> String {
    format!(
        "Discovering ONVIF devices... {}s elapsed (Ctrl-C to cancel)",
        elapsed.as_secs()
    )
}

pub(crate) fn browse_discovery(
    devices: &[DiscoveryDeviceView],
    summary: &DiscoveryResultSummary,
) -> Result<BrowserAction, AppError> {
    let mut terminal = TerminalSession::enter()?;
    let mut state = BrowserState::new(devices, DEFAULT_PAGE_SIZE, summary.total_count);
    let mut setup_form = None;

    loop {
        let (_, height) = terminal::size().map_err(terminal_error)?;
        state.set_page_size(if state.showing_details {
            panel_body_rows(height)
        } else {
            discovery_rows(height)
        });
        if let Some(form) = &setup_form {
            render_setup(&mut terminal, form)?;
        } else {
            render(&mut terminal, &mut state)?;
        }

        match event::read().map_err(terminal_error)? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                if let Some(form) = setup_form.as_mut() {
                    match form.handle_key(key) {
                        Some(SetupIntent::Cancel) => setup_form = None,
                        Some(SetupIntent::Quit) => return Ok(BrowserAction::Quit),
                        Some(SetupIntent::Submit) => {
                            let form = setup_form.take().expect("setup form should exist");
                            return Ok(BrowserAction::Add(Box::new(form.finish()?)));
                        }
                        None => {}
                    }
                } else if let Some(action) = state.handle_key(key) {
                    match action {
                        BrowserIntent::Quit => return Ok(BrowserAction::Quit),
                        BrowserIntent::Settings => number_settings(&mut terminal)?,
                        BrowserIntent::Activate(index) => {
                            let device = &devices[index].record;
                            let target = primary_target(device).ok_or_else(|| {
                                AppError::invalid_argument(
                                    "The selected discovery record has no usable device-service address.",
                                )
                            })?;
                            let suggested_id = super::suggested_device_id(target, None)?;
                            setup_form = Some(SetupForm::new(device.clone(), suggested_id));
                        }
                    }
                }
            }
            Event::Resize(_, _) => {
                state.nav.reset();
                terminal.invalidate()?;
            }
            Event::Paste(mut value) => {
                state.nav.reset();
                if let Some(form) = setup_form.as_mut() {
                    form.paste(&value);
                } else if state.filtering {
                    state.query.push_str(&value);
                    state.rebuild_filter();
                }
                value.zeroize();
            }
            _ => {}
        }
    }
}

fn number_settings(terminal: &mut TerminalSession) -> Result<(), AppError> {
    let initial = ui_settings::current()?;
    let mut view = Viewport {
        selected: LineNumbers::ALL
            .iter()
            .position(|mode| *mode == initial)
            .unwrap_or(2),
        top: 0,
    };
    let mut nav = Navigation::default();
    let mut error = String::new();
    loop {
        let (width, height) = terminal::size().map_err(terminal_error)?;
        view.clamp(4, panel_body_rows(height));
        let preview = LineNumbers::ALL[view.selected];
        draw_changed_lines(
            terminal,
            settings_frame(view, preview, &nav, &error, width, height),
        )?;
        match event::read().map_err(terminal_error)? {
            Event::Key(key) => match navigation_key(&mut nav, key, false) {
                Outcome::Action(motion) => view.apply(motion, 4, panel_body_rows(height)),
                Outcome::Unhandled => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => return Ok(()),
                    KeyCode::Enter | KeyCode::Char('s') => {
                        match ui_settings::apply(preview, key.code == KeyCode::Char('s')) {
                            Ok(()) => return Ok(()),
                            Err(failure) => error = failure.message,
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            Event::Resize(_, _) => {
                nav.reset();
                terminal.invalidate()?;
            }
            Event::Paste(_) => nav.reset(),
            _ => {}
        }
    }
}

fn settings_frame(
    view: Viewport,
    preview: LineNumbers,
    nav: &Navigation,
    error: &str,
    width: u16,
    height: u16,
) -> Vec<String> {
    let descriptions = [
        "absolute - every row shows its ordinal",
        "relative - selected row is 0; others show distance",
        "hybrid - selected ordinal; others show distance (default)",
        "off - selection marker only",
    ];
    let rows = panel_body_rows(height);
    let mut body = descriptions
        .iter()
        .enumerate()
        .skip(view.top)
        .take(rows)
        .map(|(i, text)| format!("{} {text}", if i == view.selected { '>' } else { ' ' }))
        .collect::<Vec<_>>();
    if rows >= 8 {
        body.push("Preview: selected item 12 (numbers are not device IDs)".into());
        for index in 10..13 {
            body.push(numbered_line(
                "Camera",
                index,
                11,
                40,
                width.saturating_sub(1) as usize,
                preview,
            ));
        }
    }
    panel_lines(
        "Line numbers | preview only until applied",
        &body,
        "Enter apply | s save default | Esc cancel | j/k move",
        &if error.is_empty() {
            format!(
                "SETTINGS | {}{}{}",
                preview.name(),
                if nav.pending().is_empty() {
                    String::new()
                } else {
                    format!(" | [{}]", nav.pending())
                },
                if nav.hint().is_empty() {
                    String::new()
                } else {
                    format!(" | {}", nav.hint())
                }
            )
        } else {
            format!("SAVE FAILED | {error}")
        },
        width,
        height,
    )
}

struct TerminalSession {
    stdout: io::Stdout,
    previous_lines: Vec<String>,
}

/// One alternate screen retained across the guided maintenance workflow.
pub(crate) struct Panel(TerminalSession);

impl Panel {
    pub(crate) fn enter() -> Result<Self, AppError> {
        TerminalSession::enter().map(Self)
    }

    /// Return the original record index, not its position in the filtered view.
    /// Selecting a new camera here does not register it or start the setup form.
    pub(crate) fn select_discovered_device(
        &mut self,
        devices: &[DiscoveryDeviceView],
    ) -> Result<Option<usize>, AppError> {
        let mut state = BrowserState::for_selection(devices);
        loop {
            let (_, height) = terminal::size().map_err(terminal_error)?;
            state.set_page_size(if state.showing_details {
                panel_body_rows(height)
            } else {
                discovery_rows(height)
            });
            render(&mut self.0, &mut state)?;
            match event::read().map_err(terminal_error)? {
                Event::Key(key) => match state.handle_key(key) {
                    Some(BrowserIntent::Quit) => return Ok(None),
                    Some(BrowserIntent::Activate(index)) => return Ok(Some(index)),
                    Some(BrowserIntent::Settings) => number_settings(&mut self.0)?,
                    None => {}
                },
                Event::Resize(_, _) => {
                    state.nav.reset();
                    self.0.invalidate()?;
                }
                Event::Paste(mut value) => {
                    state.nav.reset();
                    if state.filtering {
                        state.query.push_str(&value);
                        state.rebuild_filter();
                    }
                    value.zeroize();
                }
                _ => {}
            }
        }
    }

    fn draw(
        &mut self,
        title: &str,
        body: &[String],
        footer: &str,
        status: &str,
    ) -> Result<usize, AppError> {
        let (width, height) = terminal::size().map_err(terminal_error)?;
        draw_changed_lines(
            &mut self.0,
            panel_lines(title, body, footer, status, width, height),
        )?;
        Ok(panel_body_rows(height))
    }

    pub(crate) fn menu(
        &mut self,
        title: &str,
        choices: &[String],
        details: &[String],
    ) -> Result<Option<usize>, AppError> {
        self.menu_with_view(title, choices, details, &mut Viewport::default())
    }

    pub(crate) fn menu_with_view(
        &mut self,
        title: &str,
        choices: &[String],
        details: &[String],
        view: &mut Viewport,
    ) -> Result<Option<usize>, AppError> {
        if choices.is_empty() {
            self.show(title, "No items available.")?;
            return Ok(None);
        }
        let mut nav = Navigation::default();
        loop {
            let mode = ui_settings::current()?;
            let (width, height) = terminal::size().map_err(terminal_error)?;
            let rows = panel_body_rows(height);
            view.clamp(choices.len(), rows);
            draw_changed_lines(
                &mut self.0,
                menu_frame(title, choices, *view, &nav, width, height, mode),
            )?;
            match event::read().map_err(terminal_error)? {
                Event::Key(key) => match navigation_key(&mut nav, key, false) {
                    Outcome::Action(motion) => view.apply(motion, choices.len(), rows),
                    Outcome::Unhandled => match key.code {
                        KeyCode::Char('?') => {
                            nav.reset();
                            number_settings(&mut self.0)?;
                        }
                        KeyCode::Enter => return Ok(Some(view.selected)),
                        KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                        KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                            return Ok(None);
                        }
                        KeyCode::Char('i') => {
                            nav.reset();
                            self.show(
                                "Details (reported data, not measured playback)",
                                details
                                    .get(view.selected)
                                    .unwrap_or(&choices[view.selected]),
                            )?;
                        }
                        _ => {}
                    },
                    _ => {}
                },
                Event::Resize(_, _) => {
                    nav.reset();
                    self.0.invalidate()?;
                }
                Event::Paste(_) => nav.reset(),
                _ => {}
            }
        }
    }

    pub(crate) fn show(&mut self, title: &str, text: &str) -> Result<(), AppError> {
        let mut offset = 0;
        let mut nav = Navigation::default();
        loop {
            let mode = ui_settings::current()?;
            let (width, height) = terminal::size().map_err(terminal_error)?;
            let rows = panel_body_rows(height);
            let lines = numbered_text(text, width.saturating_sub(1) as usize, mode);
            offset = offset.min(lines.len().saturating_sub(rows));
            let body = lines
                .iter()
                .enumerate()
                .skip(offset)
                .take(rows)
                .map(|(i, s)| {
                    numbered_line(
                        s,
                        i,
                        offset,
                        lines.len(),
                        width.saturating_sub(1) as usize,
                        mode,
                    )
                })
                .collect::<Vec<_>>();
            self.draw(
                title,
                &body,
                "? settings | j/k 7j/3k gg/G nG | PgUp/Dn ^D/^U | Enter/Esc/q back",
                &nav_status("NORMAL (TEXT)", &nav, offset, lines.len(), mode),
            )?;
            match event::read().map_err(terminal_error)? {
                Event::Key(key) => match navigation_key(&mut nav, key, false) {
                    Outcome::Action(motion) => {
                        offset = navigation::scroll(offset, motion, lines.len(), rows)
                    }
                    Outcome::Unhandled => match key.code {
                        KeyCode::Char('?') => {
                            nav.reset();
                            number_settings(&mut self.0)?;
                        }
                        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                            return Ok(());
                        }
                        _ => {}
                    },
                    _ => {}
                },
                Event::Resize(_, _) => {
                    nav.reset();
                    self.0.invalidate()?;
                }
                Event::Paste(_) => nav.reset(),
                _ => {}
            }
        }
    }

    pub(crate) fn input(&mut self, title: &str, label: &str) -> Result<Option<String>, AppError> {
        self.input_inner(title, label, false, "")
    }

    pub(crate) fn input_with_initial(
        &mut self,
        title: &str,
        label: &str,
        initial: &str,
    ) -> Result<Option<String>, AppError> {
        self.input_inner(title, label, false, initial)
    }

    pub(crate) fn password(&mut self) -> Result<Option<String>, AppError> {
        self.input_inner(
            "Session credentials (not saved)",
            "ONVIF password:",
            true,
            "",
        )
    }

    fn input_inner(
        &mut self,
        title: &str,
        label: &str,
        secret: bool,
        initial: &str,
    ) -> Result<Option<String>, AppError> {
        let mut value = zeroize::Zeroizing::new(initial.to_owned());
        let mut cursor = value.len();
        loop {
            let (width, height) = terminal::size().map_err(terminal_error)?;
            let mut body = Vec::new();
            if panel_body_rows(height) >= 2 {
                body.push(label.to_owned());
            }
            body.push(input_view(
                &value,
                cursor,
                width.saturating_sub(1) as usize,
                secret,
            ));
            self.draw(
                title,
                &body,
                "Enter: confirm | Esc: cancel | Left/Right/Home/End: edit | Ctrl-U: clear",
                if secret {
                    "INPUT | password masked"
                } else {
                    "INPUT | literal text"
                },
            )?;
            match event::read().map_err(terminal_error)? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None);
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        value.clear();
                        cursor = 0;
                    }
                    KeyCode::Enter if !value.is_empty() => {
                        return Ok(Some(std::mem::take(&mut *value)));
                    }
                    KeyCode::Backspace => {
                        if cursor > 0 {
                            let previous = previous_boundary(&value, cursor);
                            value.drain(previous..cursor);
                            cursor = previous;
                        }
                    }
                    KeyCode::Delete if cursor < value.len() => {
                        value.remove(cursor);
                    }
                    KeyCode::Left => cursor = previous_boundary(&value, cursor),
                    KeyCode::Right if cursor < value.len() => {
                        cursor += value[cursor..]
                            .chars()
                            .next()
                            .expect("character")
                            .len_utf8()
                    }
                    KeyCode::Home => cursor = 0,
                    KeyCode::End => cursor = value.len(),
                    KeyCode::Char(c)
                        if !c.is_control() && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        value.insert(cursor, c);
                        cursor += c.len_utf8();
                    }
                    _ => {}
                },
                Event::Paste(mut text) => {
                    let clean = zeroize::Zeroizing::new(
                        text.chars().filter(|c| !c.is_control()).collect::<String>(),
                    );
                    value.insert_str(cursor, &clean);
                    cursor += clean.len();
                    text.zeroize();
                }
                Event::Resize(_, _) => self.0.invalidate()?,
                _ => {}
            }
        }
    }

    pub(crate) async fn wait<F: Future>(
        &mut self,
        title: &str,
        future: F,
    ) -> Result<Option<F::Output>, AppError> {
        let start = Instant::now();
        let mut ticker = interval(Duration::from_millis(100));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        tokio::pin!(future);
        loop {
            tokio::select! {
                biased;
                result = &mut future => return Ok(Some(result)),
                _ = ticker.tick() => {
                    self.draw(title, &[format!("Working... {}s elapsed", start.elapsed().as_secs())], "Esc/Ctrl-C: cancel; completed results remain available", &format!("BUSY | {}s elapsed", start.elapsed().as_secs()))?;
                    if event::poll(Duration::ZERO).map_err(terminal_error)? {
                        match event::read().map_err(terminal_error)? {
                            Event::Key(key) if key.kind != KeyEventKind::Release && (key.code == KeyCode::Esc || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))) => return Ok(None),
                            Event::Resize(_, _) => self.0.invalidate()?,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

// Prefer one content row to decoration in very short terminals. Keep pagination
// and rendering on the same row budget so no selectable item is hidden.
fn panel_body_rows(height: u16) -> usize {
    usize::from(height.saturating_sub(5)).max(1)
}

fn separator(width: usize) -> String {
    "─".repeat(width)
}

/// Align structured menu cells across the entire list, not only the visible page.
/// Center descriptive columns, capped at 24 display cells so one long value
/// cannot push every other row off-screen. Full values remain in item details.
/// Keep the final address column left-aligned.
pub(crate) fn aligned_menu_rows<const N: usize>(rows: &[[String; N]]) -> Vec<String> {
    let safe_rows: Vec<[String; N]> = rows
        .iter()
        .map(|row| {
            std::array::from_fn(|column| {
                let safe = row[column]
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect::<String>();
                let safe = oxvif_cli::terminal_field(&safe);
                if column + 1 == N {
                    safe
                } else {
                    truncate_to_width(&safe, 24)
                }
            })
        })
        .collect::<Vec<_>>();
    let widths: [usize; N] = std::array::from_fn(|column| {
        safe_rows
            .iter()
            .map(|row| UnicodeWidthStr::width(row[column].as_str()))
            .max()
            .unwrap_or(0)
    });
    safe_rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(column, cell)| {
                    if column + 1 == N {
                        return cell.clone();
                    }
                    let padding = widths[column] - UnicodeWidthStr::width(cell.as_str());
                    format!(
                        "{}{}{}",
                        " ".repeat(padding / 2),
                        cell,
                        " ".repeat(padding - padding / 2)
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect()
}

fn panel_lines(
    title: &str,
    body: &[String],
    footer: &str,
    status: &str,
    width: u16,
    height: u16,
) -> Vec<String> {
    let width = usize::from(width.saturating_sub(1));
    let mut lines = Vec::new();
    if height >= 5 {
        lines.push(title.to_owned());
    }
    if height >= 6 {
        lines.push(separator(width));
    }
    lines.extend(body.iter().take(panel_body_rows(height)).cloned());
    let missing = panel_body_rows(height).saturating_sub(body.len());
    lines.extend(std::iter::repeat_n(String::new(), missing));
    if height >= 4 {
        lines.push(separator(width));
    }
    if height >= 3 {
        lines.push(footer.to_owned());
    }
    if height >= 2 {
        lines.push(status_bar(status, title, width));
    }
    lines
        .into_iter()
        .take(usize::from(height))
        .map(|line| {
            let safe: String = line.chars().filter(|c| !c.is_control()).collect();
            truncate_to_width(&safe, width)
        })
        .collect()
}

// The bottom row is status, not a second help line. Add context only when the
// complete status fits, so long device names cannot hide position or pending keys.
fn status_bar(status: &str, context: &str, width: usize) -> String {
    let status = truncate_to_width(status, width);
    let used = UnicodeWidthStr::width(status.as_str());
    let gap = width.saturating_sub(used);
    if gap >= 16 {
        let context = truncate_to_width(context.split(" | ").next().unwrap_or(context), gap - 3);
        let padding = width - used - UnicodeWidthStr::width(context.as_str());
        format!("{status}{}{context}", " ".repeat(padding))
    } else {
        format!("{status}{}", " ".repeat(gap))
    }
}

// Resolve gutter width after wrapping; digit growth can only reduce content width.
fn numbered_text(text: &str, width: usize, mode: LineNumbers) -> Vec<String> {
    let mut gutter = gutter_width(1, width, mode);
    loop {
        let lines = wrap_panel_text(text, width.saturating_sub(gutter));
        let next = gutter_width(lines.len(), width, mode);
        if next <= gutter {
            return lines;
        }
        gutter = next;
    }
}

fn previous_boundary(value: &str, cursor: usize) -> usize {
    value[..cursor]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn input_view(value: &str, cursor: usize, width: usize, secret: bool) -> String {
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "│".into();
    }
    let chars = |s: &str| {
        s.chars()
            .map(|c| if secret { '*' } else { c })
            .collect::<String>()
    };
    let before = chars(&value[..cursor]);
    let after = chars(&value[cursor..]);
    let mut prefix = before.clone();
    if UnicodeWidthStr::width(before.as_str()) >= width {
        let mut tail = Vec::new();
        let mut used = 0;
        for c in before.chars().rev() {
            let size = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + size > width.saturating_sub(2) {
                break;
            }
            used += size;
            tail.push(c);
        }
        prefix = format!("…{}", tail.into_iter().rev().collect::<String>());
    }
    let used = UnicodeWidthStr::width(prefix.as_str()) + 1;
    format!(
        "{prefix}│{}",
        truncate_to_width(&after, width.saturating_sub(used))
    )
}

fn wrap_panel_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut row = String::new();
        let mut used = 0;
        let line = line.chars().filter(|c| !c.is_control()).collect::<String>();
        let line = oxvif_cli::terminal_field(&line);
        for c in line.chars() {
            let size = UnicodeWidthChar::width(c).unwrap_or(0);
            if used + size > width.max(1) && !row.is_empty() {
                lines.push(std::mem::take(&mut row));
                used = 0;
            }
            row.push(c);
            used += size;
        }
        lines.push(row);
    }
    lines
}

impl TerminalSession {
    fn invalidate(&mut self) -> Result<(), AppError> {
        execute!(self.stdout, Clear(ClearType::All)).map_err(terminal_error)?;
        self.previous_lines.clear();
        Ok(())
    }
    fn enter() -> Result<Self, AppError> {
        ui_settings::current()?;
        enable_raw_mode().map_err(terminal_error)?;
        let mut session = Self {
            stdout: io::stdout(),
            previous_lines: Vec::new(),
        };
        // The guard restores raw/alternate-screen state even if setup only partly succeeds.
        execute!(session.stdout, EnterAlternateScreen, Hide).map_err(terminal_error)?;
        // Windows' native key-event backend cannot distinguish paste from typing.
        // Do not enable a paste protocol that this backend cannot decode.
        #[cfg(unix)]
        execute!(session.stdout, EnableBracketedPaste).map_err(terminal_error)?;
        Ok(session)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = execute!(self.stdout, DisableBracketedPaste);
        let _ = execute!(
            self.stdout,
            EndSynchronizedUpdate,
            Show,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
    }
}

struct BrowserState<'a> {
    purpose: DiscoveryPurpose,
    devices: &'a [DiscoveryDeviceView],
    total_count: usize,
    filtered: Vec<usize>,
    selected: usize,
    top: usize,
    nav: Navigation,
    page_size: usize,
    query: String,
    filtering: bool,
    registration_view: RegistrationView,
    showing_details: bool,
    detail_scroll: usize,
    detail_max_scroll: usize,
}

impl<'a> BrowserState<'a> {
    fn new(devices: &'a [DiscoveryDeviceView], page_size: usize, total_count: usize) -> Self {
        Self {
            purpose: DiscoveryPurpose::Add,
            devices,
            total_count,
            filtered: (0..devices.len()).collect(),
            selected: 0,
            top: 0,
            nav: Navigation::default(),
            page_size: page_size.max(1),
            query: String::new(),
            filtering: false,
            registration_view: RegistrationView::All,
            showing_details: false,
            detail_scroll: 0,
            detail_max_scroll: 0,
        }
    }

    fn for_selection(devices: &'a [DiscoveryDeviceView]) -> Self {
        Self {
            purpose: DiscoveryPurpose::Select,
            ..Self::new(devices, DEFAULT_PAGE_SIZE, devices.len())
        }
    }

    fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
        self.clamp_selection();
    }

    fn rebuild_filter(&mut self) {
        self.nav.reset();
        self.top = 0;
        self.filtered = self
            .devices
            .iter()
            .enumerate()
            .filter(|(_, device)| {
                let registration_matches = match self.registration_view {
                    RegistrationView::All => true,
                    RegistrationView::Saved => {
                        device.registration_status == DiscoveryRegistrationStatus::Saved
                    }
                    RegistrationView::Unregistered => {
                        device.registration_status != DiscoveryRegistrationStatus::Saved
                    }
                };
                if !registration_matches {
                    return false;
                }
                discovery_query_matches(device, &self.query)
            })
            .map(|(index, _)| index)
            .collect();
        self.selected = 0;
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        let mut view = Viewport {
            selected: self.selected,
            top: self.top,
        };
        view.clamp(self.filtered.len(), self.page_size);
        self.selected = view.selected;
        self.top = view.top;
    }

    fn page_start(&self) -> usize {
        self.top
    }

    fn current(&self) -> Option<&'a DiscoveryDeviceView> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.devices.get(*index))
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<BrowserIntent> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.nav.reset();
            return Some(BrowserIntent::Quit);
        }
        if self.filtering {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.query.pop();
                    self.rebuild_filter();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.query.clear();
                    self.rebuild_filter();
                }
                KeyCode::Char(character)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.query.push(character);
                    self.rebuild_filter();
                }
                _ => {}
            }
            return None;
        }

        match navigation_key(&mut self.nav, key, true) {
            Outcome::Action(motion) => {
                if self.showing_details {
                    self.detail_scroll = navigation::scroll(
                        self.detail_scroll,
                        motion,
                        self.detail_max_scroll.saturating_add(self.page_size),
                        self.page_size,
                    );
                } else {
                    let mut view = Viewport {
                        selected: self.selected,
                        top: self.top,
                    };
                    view.apply(motion, self.filtered.len(), self.page_size);
                    self.selected = view.selected;
                    self.top = view.top;
                }
                return None;
            }
            Outcome::Unhandled => {}
            _ => return None,
        }
        if key.code == KeyCode::Char('?') {
            self.nav.reset();
            return Some(BrowserIntent::Settings);
        }
        if self.showing_details {
            match key.code {
                KeyCode::Esc | KeyCode::Char('i') => {
                    self.nav.reset();
                    self.showing_details = false;
                    self.detail_scroll = 0;
                }
                KeyCode::Char('q') => return Some(BrowserIntent::Quit),
                _ => {}
            }
            return None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(BrowserIntent::Quit),
            KeyCode::Char('/') => {
                self.filtering = true;
                None
            }
            KeyCode::Char('c') => {
                self.query.clear();
                self.rebuild_filter();
                None
            }
            KeyCode::Char('r') => {
                self.registration_view = if self.registration_view == RegistrationView::Saved {
                    RegistrationView::All
                } else {
                    RegistrationView::Saved
                };
                self.rebuild_filter();
                None
            }
            KeyCode::Char('n') => {
                self.registration_view = if self.registration_view == RegistrationView::Unregistered
                {
                    RegistrationView::All
                } else {
                    RegistrationView::Unregistered
                };
                self.rebuild_filter();
                None
            }
            KeyCode::Char('A') => {
                self.registration_view = RegistrationView::All;
                self.rebuild_filter();
                None
            }
            KeyCode::Char('i') => {
                if self.current().is_some() {
                    self.showing_details = true;
                    self.detail_scroll = 0;
                }
                None
            }
            KeyCode::Enter | KeyCode::Char('a') => {
                let device = self.current()?;
                let eligible = match self.purpose {
                    DiscoveryPurpose::Select => key.code == KeyCode::Enter,
                    DiscoveryPurpose::Add => {
                        device.registration_status != DiscoveryRegistrationStatus::Saved
                            && primary_target(device).is_some()
                    }
                };
                eligible.then(|| BrowserIntent::Activate(self.filtered[self.selected]))
            }
            _ => None,
        }
    }
}

fn render_setup(terminal: &mut TerminalSession, form: &SetupForm) -> Result<(), AppError> {
    let (width, height) = terminal::size().map_err(terminal_error)?;
    let rows = panel_body_rows(height);
    let mut body = Vec::new();
    if rows >= 7 {
        body.push(format!("Device: {}", primary_address(&form.device)));
        body.push(format!(
            "Target: {}",
            primary_target(&form.device).unwrap_or("(no usable address)")
        ));
        body.push(separator(width.saturating_sub(1) as usize));
    }
    for (label, value, field, secret) in [
        ("Device ID", form.id.as_str(), SetupField::Id, false),
        (
            "Username",
            form.username.as_str(),
            SetupField::Username,
            false,
        ),
        (
            "Password",
            form.password.as_str(),
            SetupField::Password,
            true,
        ),
    ] {
        if rows >= 3 || field == form.field {
            body.push(setup_field_line(label, value, form.field, field, secret));
        }
    }
    body.push(
        form.error
            .as_deref()
            .unwrap_or("Nothing saved until setup succeeds.")
            .to_owned(),
    );
    draw_changed_lines(
        terminal,
        panel_lines(
            "oxvif discovery - onboard selected device",
            &body,
            "Tab/Up/Down field | Enter next/submit | Ctrl-U clear | Esc back",
            "INPUT | credentials masked",
            width,
            height,
        ),
    )
}

fn setup_field_line(
    label: &str,
    value: &str,
    active: SetupField,
    field: SetupField,
    password: bool,
) -> String {
    let marker = if active == field { '>' } else { ' ' };
    let mut display = if password {
        "•".repeat(value.chars().count())
    } else {
        value.to_owned()
    };
    if marker == '>' {
        display.push('_');
    }
    format!("{marker} {label:<10} {display}")
}

fn discovery_rows(height: u16) -> usize {
    let rows = panel_body_rows(height);
    if rows >= 4 {
        (rows - 3).clamp(1, DEFAULT_PAGE_SIZE)
    } else {
        rows
    }
}

fn render(terminal: &mut TerminalSession, state: &mut BrowserState<'_>) -> Result<(), AppError> {
    if state.showing_details {
        return render_details(terminal, state);
    }
    let (width, height) = terminal::size().map_err(terminal_error)?;
    let mode = ui_settings::current()?;
    state.set_page_size(discovery_rows(height));
    draw_changed_lines(terminal, discovery_frame(state, width, height, mode))
}

fn discovery_frame(
    state: &BrowserState<'_>,
    width: u16,
    height: u16,
    mode: LineNumbers,
) -> Vec<String> {
    let columns = width.saturating_sub(1) as usize;
    let saved = state
        .devices
        .iter()
        .filter(|d| d.registration_status == DiscoveryRegistrationStatus::Saved)
        .count();
    let title = format!(
        "{} | {} found | {} saved | {} shown",
        if state.purpose == DiscoveryPurpose::Select {
            "oxvif manage discovery"
        } else {
            "oxvif discovery"
        },
        state.total_count,
        saved,
        state.filtered.len()
    );
    let mut body = Vec::new();
    if panel_body_rows(height) >= 4 {
        body.push(format!(
            "View: {} | Search: {}",
            registration_view_name(state.registration_view),
            if state.query.is_empty() {
                "(none)"
            } else {
                &state.query
            }
        ));
        body.push(format!(
            "{}RECORD STATUS     ADDRESS              DEVICE                  SAVED AS",
            " ".repeat(gutter_width(state.filtered.len(), columns, mode))
        ));
    }
    if state.filtered.is_empty() {
        body.push("No devices match the current filter.".into());
    } else {
        for (position, index) in state
            .filtered
            .iter()
            .enumerate()
            .skip(state.page_start())
            .take(state.page_size)
        {
            let device = &state.devices[*index];
            let text = format!(
                "{:<6} {} {} {} {}",
                index + 1,
                fit_cell(
                    &device.registration_status.as_str().to_ascii_uppercase(),
                    10
                ),
                fit_cell(&primary_address(device), 20),
                fit_cell(&discovery_device_label(device), 23),
                device.registered_device_id.as_deref().unwrap_or("-")
            );
            body.push(numbered_line(
                &text,
                position,
                state.selected,
                state.filtered.len(),
                columns,
                mode,
            ));
        }
    }
    if panel_body_rows(height) >= 4 {
        body.push(
            state
                .current()
                .map(|d| {
                    format!(
                        "{} | {}",
                        if state.purpose == DiscoveryPurpose::Select
                            && d.registration_status != DiscoveryRegistrationStatus::Saved
                        {
                            "Session only (not saved)".to_owned()
                        } else {
                            format!(
                                "Saved as: {}",
                                d.registered_device_id.as_deref().unwrap_or("(not saved)")
                            )
                        },
                        display_endpoint(d)
                    )
                })
                .unwrap_or_default(),
        );
    }
    panel_lines(
        &title,
        &body,
        if state.filtering {
            "Type search | Enter/Esc back | Ctrl-U clear | Ctrl-C close"
        } else if state.purpose == DiscoveryPurpose::Select {
            "? settings | j/k gg/G ^D/^U | / r n A filter | c clear | i info | Enter select | q back"
        } else {
            "? settings | j/k gg/G ^D/^U | / r n A filter | c clear | i info | Enter add | q quit"
        },
        &nav_status(
            if state.filtering { "SEARCH" } else { "NORMAL" },
            &state.nav,
            state.selected,
            state.filtered.len(),
            mode,
        ),
        width,
        height,
    )
}

fn render_details(
    terminal: &mut TerminalSession,
    state: &mut BrowserState<'_>,
) -> Result<(), AppError> {
    let (width, height) = terminal::size().map_err(terminal_error)?;
    let Some(device) = state.current() else {
        state.showing_details = false;
        return render(terminal, state);
    };
    let mode = ui_settings::current()?;
    state.page_size = panel_body_rows(height);
    let content = numbered_text(
        &discovery_detail_lines(device, usize::MAX).join("\n"),
        width.saturating_sub(1) as usize,
        mode,
    );
    state.detail_max_scroll = content.len().saturating_sub(state.page_size);
    state.detail_scroll = state.detail_scroll.min(state.detail_max_scroll);
    let body = content
        .iter()
        .enumerate()
        .skip(state.detail_scroll)
        .take(state.page_size)
        .map(|(i, line)| {
            numbered_line(
                line,
                i,
                state.detail_scroll,
                content.len(),
                width.saturating_sub(1) as usize,
                mode,
            )
        })
        .collect::<Vec<_>>();
    draw_changed_lines(
        terminal,
        panel_lines(
            "oxvif discovery - device details",
            &body,
            "? settings | j/k gg/G nG | h/l PgUp/Dn ^D/^U | i/Esc back | q quit",
            &nav_status(
                "NORMAL (TEXT)",
                &state.nav,
                state.detail_scroll,
                content.len(),
                mode,
            ),
            width,
            height,
        ),
    )
}

fn discovery_detail_lines(device: &DiscoveryDeviceView, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    {
        let mut field = |label: &str, value: &str| {
            extend_wrapped(&mut lines, &format!("{label}: {value}"), width);
        };

        field("Status", device.registration_status.as_str());
        field(
            "Saved as",
            device.registered_device_id.as_deref().unwrap_or("-"),
        );
        field("Address", &primary_address(device));
        field(
            "Device service",
            primary_target(device).unwrap_or("(no usable address)"),
        );
        field(
            "Manufacturer",
            device.manufacturer.as_deref().unwrap_or("-"),
        );
        field("Model", device.model.as_deref().unwrap_or("-"));
        field(
            "Firmware",
            device.firmware_version.as_deref().unwrap_or("-"),
        );
        field("Serial", device.serial_number.as_deref().unwrap_or("-"));
        field("Endpoint UUID", display_endpoint(device));
    }
    extend_detail_collection(&mut lines, "Types", &device.types, width);
    extend_detail_collection(&mut lines, "XAddrs", &device.xaddrs, width);
    extend_detail_collection(&mut lines, "Scopes", &device.scopes, width);
    lines
}

fn extend_detail_collection(lines: &mut Vec<String>, label: &str, values: &[String], width: usize) {
    if values.is_empty() {
        extend_wrapped(lines, &format!("{label}: -"), width);
        return;
    }
    extend_wrapped(lines, &format!("{label}:"), width);
    for value in values {
        extend_wrapped(lines, &format!("  - {value}"), width);
    }
}

fn extend_wrapped(lines: &mut Vec<String>, value: &str, width: usize) {
    let width = width.max(1);
    let mut line = String::new();
    let mut used = 0usize;
    for character in value.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used > 0 && used + character_width > width {
            lines.push(std::mem::take(&mut line));
            used = 0;
        }
        line.push(character);
        used += character_width;
    }
    lines.push(line);
}

fn registration_view_name(view: RegistrationView) -> &'static str {
    match view {
        RegistrationView::All => "all",
        RegistrationView::Saved => "saved",
        RegistrationView::Unregistered => "unregistered",
    }
}

fn discovery_device_label(device: &DiscoveryDeviceView) -> String {
    match (device.manufacturer.as_deref(), device.model.as_deref()) {
        (Some(manufacturer), Some(model)) => format!("{manufacturer} {model}"),
        (Some(manufacturer), None) => manufacturer.to_owned(),
        (None, Some(model)) => model.to_owned(),
        (None, None) => "Not advertised".to_owned(),
    }
}

fn draw_changed_lines(terminal: &mut TerminalSession, lines: Vec<String>) -> Result<(), AppError> {
    let (width, height) = terminal::size().map_err(terminal_error)?;
    let lines = lines
        .into_iter()
        .take(height as usize)
        .map(|line| {
            let safe: String = line.chars().filter(|c| !c.is_control()).collect();
            truncate_to_width(&safe, width.saturating_sub(1) as usize)
        })
        .collect::<Vec<_>>();
    queue!(terminal.stdout, BeginSynchronizedUpdate).map_err(terminal_error)?;
    let row_count = lines
        .len()
        .max(terminal.previous_lines.len())
        .min(height as usize);
    for row in 0..row_count {
        let current = lines.get(row).map_or("", String::as_str);
        let previous = terminal.previous_lines.get(row).map_or("", String::as_str);
        if current != previous {
            let status_row = height >= 2 && row + 1 == usize::from(height);
            queue!(
                terminal.stdout,
                MoveTo(0, u16::try_from(row).unwrap_or(u16::MAX)),
                SetAttribute(if status_row {
                    Attribute::Reverse
                } else {
                    Attribute::Reset
                }),
                Print(current),
                SetAttribute(Attribute::Reset),
                Clear(ClearType::UntilNewLine)
            )
            .map_err(terminal_error)?;
        }
    }
    queue!(terminal.stdout, EndSynchronizedUpdate).map_err(terminal_error)?;
    terminal.stdout.flush().map_err(terminal_error)?;
    terminal.previous_lines = lines;
    Ok(())
}

fn primary_address(device: &DiscoveryRecord) -> String {
    primary_target(device)
        .and_then(|target| {
            url::Url::parse(target)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
        })
        .unwrap_or_else(|| "(no address)".to_owned())
}

fn primary_target(device: &DiscoveryRecord) -> Option<&str> {
    device
        .xaddrs
        .iter()
        .find(|target| normalize_target(target).is_ok())
        .map(String::as_str)
}

fn display_endpoint(device: &DiscoveryRecord) -> &str {
    if device.endpoint.trim().is_empty() {
        "(no endpoint)"
    } else {
        &device.endpoint
    }
}

fn fit_cell(value: &str, width: usize) -> String {
    let value = truncate_to_width(value, width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(value.as_str()));
    format!("{value}{}", " ".repeat(padding))
}

fn truncate_to_width(value: &str, width: usize) -> String {
    // Escape before measuring: directional controls can reorder otherwise
    // printable camera data, and their visible escape occupies display cells.
    let value = oxvif_cli::terminal_field(value);
    if UnicodeWidthStr::width(value.as_str()) <= width {
        return value;
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    let mut output = String::new();
    let mut used = 0usize;
    for character in value.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used + character_width > width - 1 {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output.push('…');
    output
}

fn terminal_error(error: io::Error) -> AppError {
    AppError::internal(format!("Interactive terminal failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::Motion;

    #[test]
    fn manage_discovery_filters_select_original_records_without_onboarding() {
        let devices = vec![
            view("192.0.2.1", "Other", "Camera", None),
            view("192.0.2.2", "Acme", "Camera", Some("front-door")),
            view("192.0.2.3", "Acme", "Camera", None),
        ];
        let mut state = BrowserState::for_selection(&devices);
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        for c in "/Acme".chars() {
            assert!(state.handle_key(key(c)).is_none());
        }
        assert!(state.filtering);
        assert_eq!(state.filtered, [1, 2]);
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(
            state.handle_key(enter).is_none(),
            "finish search, not select"
        );
        state.handle_key(key('r'));
        assert_eq!(state.filtered, [1]);
        assert!(matches!(
            state.handle_key(enter),
            Some(BrowserIntent::Activate(1))
        ));
        state.handle_key(key('n'));
        assert_eq!(state.filtered, [2]);
        assert!(
            state.handle_key(key('a')).is_none(),
            "manage never onboards"
        );
        assert!(matches!(
            state.handle_key(enter),
            Some(BrowserIntent::Activate(2))
        ));
        let frame = discovery_frame(&state, 120, 24, LineNumbers::Hybrid).join("\n");
        assert!(frame.contains("oxvif manage discovery"));
        assert!(frame.contains("Session only (not saved)"));
        assert!(frame.contains("Enter select"));
        assert!(!frame.contains("Enter add"));
        state.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(state.filtered, [1, 2], "A preserves the text query");
        state.handle_key(key('c'));
        assert_eq!(state.filtered, [0, 1, 2]);
        for c in "/123ggjk".chars() {
            state.handle_key(key(c));
        }
        assert_eq!(state.query, "123ggjk", "search bypasses Vim navigation");
        assert!(state.filtered.is_empty());
        assert!(state.handle_key(enter).is_none());
        assert!(
            state.handle_key(enter).is_none(),
            "empty results cannot select"
        );
        state.handle_key(key('/'));
        state.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(state.filtered, [0, 1, 2]);
        assert!(
            state
                .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                .is_none()
        );
        assert!(matches!(
            state.handle_key(key('q')),
            Some(BrowserIntent::Quit)
        ));
    }

    #[test]
    fn interactive_frames_escape_directional_data_before_measuring_width() {
        let literal = "Camera\u{202e}txt\u{2066}end";
        let expected = "Camera\\u{202e}txt\\u{2066}end";
        let choices = vec![literal.to_owned()];
        let frame = menu_frame(
            literal,
            &choices,
            Viewport::default(),
            &Navigation::default(),
            100,
            12,
            LineNumbers::Hybrid,
        );
        assert_eq!(frame[0], expected);
        assert!(frame[2].contains(expected));
        assert_eq!(
            choices[0], literal,
            "display escaping must not change selection data"
        );
        let wrapped = wrap_panel_text(literal, 12);
        assert_eq!(
            wrapped.concat(),
            expected,
            "escape before wrapping, without losing text"
        );
        assert!(
            wrapped
                .iter()
                .all(|line| UnicodeWidthStr::width(line.as_str()) <= 12)
        );
        assert_eq!(truncate_to_width(literal, 12), "Camera\\u{20…");
        let rows = aligned_menu_rows(&[[literal.to_owned(), "address".into()]]);
        assert!(!rows[0].contains('\u{202e}'));
        assert!(!rows[0].contains('\u{2066}'));
    }

    #[test]
    fn discovery_vim_counts_status_and_compact_frames() {
        let devices = (1..=40)
            .map(|i| view(&format!("192.0.2.{i}"), "廠牌", "Camera", None))
            .collect::<Vec<_>>();
        let mut state = BrowserState::new(&devices, 10, 40);
        for c in "21G".chars() {
            state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(state.selected, 20);
        state.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(state.selected, 20);
        assert_eq!(state.nav.pending(), "g");
        assert!(
            discovery_frame(&state, 80, 24, LineNumbers::Hybrid)
                .last()
                .unwrap()
                .contains("[g]")
        );
        state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(state.selected, 20);
        assert_eq!(state.nav.pending(), "");
        for c in "gg7j3k".chars() {
            state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(state.selected, 4);
        state.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        state.query = "missing-model".into();
        state.rebuild_filter();
        assert_eq!(state.nav.pending(), "");
        assert!(
            discovery_frame(&state, 80, 24, LineNumbers::Hybrid)
                .last()
                .unwrap()
                .contains("item 0/0")
        );
        state.query.clear();
        state.rebuild_filter();
        for (width, height) in [(80, 24), (40, 8), (16, 4), (8, 1), (0, 0)] {
            state.set_page_size(discovery_rows(height));
            let lines = discovery_frame(&state, width, height, LineNumbers::Hybrid);
            assert_eq!(lines.len(), height as usize);
            assert!(lines.iter().all(|s| UnicodeWidthStr::width(s.as_str()) <= width.saturating_sub(1) as usize));
            if width > 2 && height > 0 {
                assert!(lines.iter().any(|s| s.starts_with('>')));
            }
        }
    }

    #[test]
    fn line_number_rendering_and_settings_respect_modes_and_input_boundaries() {
        for (mode, selected, next) in [
            (LineNumbers::Absolute, "> 21 Camera", "  22 Camera"),
            (LineNumbers::Relative, ">  0 Camera", "   1 Camera"),
            (LineNumbers::Hybrid, "> 21 Camera", "   1 Camera"),
            (LineNumbers::Off, "> Camera", "  Camera"),
        ] {
            assert_eq!(numbered_line("Camera", 20, 20, 40, 80, mode), selected);
            assert_eq!(numbered_line("Camera", 21, 20, 40, 80, mode), next);
            for width in [0, 1, 8, 24, 80] {
                let lines = numbered_text(&"Unicode 攝影機 ".repeat(40), width, mode);
                for (i, line) in lines.iter().enumerate() {
                    let rendered = truncate_to_width(
                        &numbered_line(line, i, 20, lines.len(), width, mode),
                        width,
                    );
                    assert!(UnicodeWidthStr::width(rendered.as_str()) <= width);
                }
            }
            let frame = settings_frame(
                Viewport::default(),
                mode,
                &Navigation::default(),
                "",
                80,
                24,
            )
            .join("\n");
            assert!(frame.contains(&format!("SETTINGS | {}", mode.name())));
            assert!(frame.contains("s save default"));
        }
        let mut browser = BrowserState::new(&[], 10, 0);
        let settings = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert!(matches!(
            browser.handle_key(settings),
            Some(BrowserIntent::Settings)
        ));
        browser.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert!(browser.handle_key(settings).is_none());
        assert_eq!(browser.nav.pending(), "");
        browser.filtering = true;
        assert!(browser.handle_key(settings).is_none());
        assert_eq!(browser.query, "?");
        for (width, height) in [(0, 0), (8, 1), (24, 6), (80, 24)] {
            let mut view = Viewport {
                selected: 3,
                top: 0,
            };
            view.clamp(4, panel_body_rows(height));
            let frame = settings_frame(
                view,
                LineNumbers::Off,
                &Navigation::default(),
                "",
                width,
                height,
            );
            assert_eq!(frame.len(), height as usize);
            assert!(frame.iter().all(|s| UnicodeWidthStr::width(s.as_str()) <= width.saturating_sub(1) as usize));
            if width > 1 && height > 0 {
                assert!(frame.iter().any(|s| s.starts_with("> off")));
            }
        }
    }

    #[test]
    fn text_gutters_and_pending_status_preserve_width_and_input_modes() {
        let text = "攝影機 profile data\n".repeat(100);
        for width in [0, 1, 8, 16, 24, 80] {
            let lines = numbered_text(&text, width, LineNumbers::Hybrid);
            for (index, line) in lines.iter().enumerate() {
                let numbered = truncate_to_width(
                    &numbered_line(line, index, 20, lines.len(), width, LineNumbers::Hybrid),
                    width,
                );
                assert!(UnicodeWidthStr::width(numbered.as_str()) <= width);
            }
        }
        let mut nav = Navigation::default();
        for c in "123456g".chars() {
            nav.feed(NavKey::Char(c));
        }
        assert!(
            truncate_to_width(&nav_status("NORMAL", &nav, 0, 40, LineNumbers::Hybrid), 24)
                .contains("123456g")
        );
        let mut form = SetupForm::new(record("192.0.2.1", "Example", "Camera"), String::new());
        for c in "123ggjk".chars() {
            form.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(form.id, "123ggjk");
        form.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(form.id, "");
    }

    fn profile_lines(choices: &[String], selected: usize, width: u16, height: u16) -> Vec<String> {
        let mut view = Viewport {
            selected,
            top: selected,
        };
        view.clamp(choices.len(), panel_body_rows(height));
        menu_frame(
            "Select media profile",
            choices,
            view,
            &Navigation::default(),
            width,
            height,
            LineNumbers::Hybrid,
        )
    }

    #[test]
    fn menu_columns_center_to_longest_cell_across_pages_using_display_width() {
        let rows = aligned_menu_rows(&[
            ["new".into(), "http://192.0.2.1/onvif".into()],
            ["saved".into(), "http://192.0.2.2/onvif".into()],
        ]);
        assert_eq!(
            rows,
            [
                " new  | http://192.0.2.1/onvif",
                "saved | http://192.0.2.2/onvif"
            ]
        );
        let rows = aligned_menu_rows(&[
            ["門\x1b".into(), "a".into(), "http://192.0.2.1".into()],
            ["Camera".into(), "front".into(), "http://192.0.2.2".into()],
        ]);
        assert_eq!(
            rows,
            [
                "  門   |   a   | http://192.0.2.1",
                "Camera | front | http://192.0.2.2"
            ]
        );
        let first = profile_lines(&rows, 0, 80, 5);
        let second = profile_lines(&rows, 1, 80, 5);
        for line in [
            first.iter().find(|s| s.starts_with('>')).unwrap(),
            second.iter().find(|s| s.starts_with('>')).unwrap(),
        ] {
            let prefix = line.split("http://").next().unwrap();
            assert_eq!(UnicodeWidthStr::width(prefix), 21);
        }
        assert!(aligned_menu_rows::<2>(&[]).is_empty());
    }

    #[test]
    fn long_name_does_not_hide_other_camera_identities() {
        let rows = aligned_menu_rows(&[
            [
                "Cam 1".into(),
                "nav-01".into(),
                "http://127.0.0.1/onvif".into(),
            ],
            [
                "攝影機長名稱".repeat(80),
                "nav-wrap".into(),
                "http://127.0.0.2/onvif".into(),
            ],
        ]);
        for width in [48, 80, 120] {
            let frame = profile_lines(&rows, 0, width, 24);
            let selected = frame.iter().find(|s| s.starts_with('>')).unwrap();
            assert!(selected.contains("Cam 1"), "{selected}");
            assert!(selected.contains("nav-01"), "{selected}");
            if width >= 80 {
                assert!(selected.contains("http://127.0.0.1/onvif"));
            }
        }
        assert!(rows[1].contains('…'));
    }

    #[test]
    fn adapter_sequences_modifiers_and_repeats() {
        let mut nav = Navigation::default();
        let mut view = Viewport::default();
        for c in "12j".chars() {
            match navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
                false,
            ) {
                Outcome::Action(motion) => view.apply(motion, 40, 10),
                outcome => assert_eq!(outcome, Outcome::Pending),
            }
        }
        assert_eq!(
            view,
            Viewport {
                selected: 12,
                top: 3
            }
        );
        assert_eq!(
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                false
            ),
            Outcome::Action(Motion::HalfDown(1))
        );
        assert_eq!(
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT),
                false
            ),
            Outcome::Consumed
        );
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            assert_eq!(
                navigation_key(
                    &mut nav,
                    KeyEvent::new_with_kind(KeyCode::Char('g'), KeyModifiers::NONE, kind),
                    false
                ),
                Outcome::Consumed
            );
            assert_eq!(nav.pending(), "");
        }
        assert_eq!(
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
                false
            ),
            Outcome::Pending
        );
        assert_eq!(
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                false
            ),
            Outcome::Consumed
        );
        assert_eq!(
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                false
            ),
            Outcome::Unhandled
        );
    }

    #[test]
    fn discovery_half_pages_work_in_list_and_details_but_filter_control_u_clears() {
        let devices = (1..=25)
            .map(|i| view(&format!("192.0.2.{i}"), "Example", "Camera", None))
            .collect::<Vec<_>>();
        let mut state = BrowserState::new(&devices, 10, devices.len());
        let down = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let up = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        state.handle_key(down);
        assert_eq!(state.selected, 5);
        state.handle_key(down);
        assert_eq!(state.selected, 10);
        state.handle_key(up);
        assert_eq!(state.selected, 5);
        state.showing_details = true;
        state.detail_max_scroll = 8;
        state.handle_key(down);
        assert_eq!(state.detail_scroll, 5);
        state.handle_key(down);
        assert_eq!(state.detail_scroll, 8);
        state.handle_key(up);
        assert_eq!(state.detail_scroll, 3);
        assert_eq!(state.selected, 5);
        state.showing_details = false;
        state.filtering = true;
        state.query = "Example".into();
        state.handle_key(down);
        assert_eq!(state.query, "Example");
        assert_eq!(state.selected, 5);
        state.handle_key(up);
        assert!(state.query.is_empty());
        assert_eq!(state.filtered.len(), 25);
        let mut empty = BrowserState::new(&[], 10, 0);
        empty.handle_key(down);
        empty.handle_key(up);
        assert_eq!(empty.selected, 0);
    }

    #[test]
    fn panel_separates_content_help_and_status_with_bounded_rows() {
        assert_eq!(
            panel_lines("Title", &["Camera".into()], "Esc back", "NORMAL", 12, 6),
            [
                "Title",
                "───────────",
                "Camera",
                "───────────",
                "Esc back",
                "NORMAL     "
            ]
        );
        for width in [0, 1, 2, 12, 80, 120] {
            for height in 0..30 {
                let body = vec!["> 攝影機\x1b".to_owned(); 40];
                let lines = panel_lines("Title", &body, "Esc back", "NORMAL", width, height);
                assert_eq!(lines.len(), height as usize);
                assert!(lines.iter().all(|l| UnicodeWidthStr::width(l.as_str())
                    <= width.saturating_sub(1) as usize
                    && !l.chars().any(char::is_control)));
                if width > 2 && height > 0 {
                    assert_eq!(
                        lines.iter().filter(|l| l.starts_with('>')).count(),
                        panel_body_rows(height)
                    );
                }
                if width >= 12 && height >= 4 {
                    assert_eq!(lines[lines.len() - 3], separator(width as usize - 1));
                    assert_eq!(lines[lines.len() - 2], "Esc back");
                    let status = lines.last().unwrap();
                    assert!(status.starts_with("NORMAL"));
                    assert_eq!(UnicodeWidthStr::width(status.as_str()), width as usize - 1);
                    assert!(!status.contains("Esc back"));
                    if width >= 80 {
                        assert!(status.ends_with("Title"));
                    }
                }
            }
        }
    }

    #[test]
    fn guided_input_scrolls_to_cursor_and_never_displays_secrets() {
        let path = "C:/long-parent/攝影機/very-long-folder/snapshot.jpg";
        let end = input_view(path, path.len(), 20, false);
        assert!(end.ends_with("snapshot.jpg│"), "{end}");
        assert!(end.starts_with('…'));
        assert!(input_view(path, 0, 20, false).starts_with("│C:/"));
        for width in [0, 1, 2, 8, 20, 80] {
            assert!(
                UnicodeWidthStr::width(input_view(path, path.len(), width, false).as_str())
                    <= width
            );
            let masked = input_view("private密碼", "private密碼".len(), width, true);
            assert!(!masked.contains("private") && !masked.contains('密'));
        }
        assert_eq!(previous_boundary("a密", "a密".len()), 1);
        assert_eq!(previous_boundary("a密", 1), 0);
        assert_eq!(
            wrap_panel_text("AB攝影機\n\x1btext", 4),
            vec!["AB攝", "影機", "text"]
        );
    }

    #[test]
    fn pending_sequences_never_activate_application_actions() {
        for action in [
            KeyCode::Enter,
            KeyCode::Char('i'),
            KeyCode::Char('q'),
            KeyCode::Char('/'),
        ] {
            let mut nav = Navigation::default();
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
                false,
            );
            assert_eq!(
                navigation_key(&mut nav, KeyEvent::new(action, KeyModifiers::NONE), false),
                Outcome::Consumed
            );
            assert_eq!(nav.hint(), "Unsupported sequence; cancelled");
            assert_eq!(nav.pending(), "");
        }
        let mut nav = Navigation::default();
        nav.feed(NavKey::Char('g'));
        assert_eq!(
            navigation_key(
                &mut nav,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                false
            ),
            Outcome::Unhandled
        );
        assert_eq!(nav.pending(), "");
        assert_eq!(
            nav_status("NORMAL", &nav, 0, 0, LineNumbers::Hybrid),
            "NORMAL | item 0/0 | numbers:hybrid"
        );
    }

    #[test]
    fn profile_pages_fit_resized_unicode_terminals() {
        let choices = (0..40)
            .map(|n| format!("攝影機 {n} (token-{n})\x1b\n"))
            .collect::<Vec<_>>();
        for (width, height) in [(80, 24), (32, 6), (12, 3), (8, 1), (0, 0)] {
            let lines = profile_lines(&choices, 29, width, height);
            assert!(lines.len() <= usize::from(height));
            assert!(lines.iter().all(
                |l| UnicodeWidthStr::width(l.as_str()) <= usize::from(width.saturating_sub(1))
            ));
            assert!(lines.iter().all(|l| !l.chars().any(char::is_control)));
            if width > 1 && height > 0 {
                assert!(lines.iter().any(|l| l.starts_with('>')));
            }
        }
        assert!(
            profile_lines(&choices, 29, 80, 6)
                .iter()
                .any(|l| l.starts_with("> 30 攝影機 29"))
        );
    }

    #[test]
    fn discovery_progress_reports_whole_elapsed_seconds() {
        assert_eq!(
            discovery_progress_text(Duration::from_millis(3_900)),
            "Discovering ONVIF devices... 3s elapsed (Ctrl-C to cancel)"
        );
    }

    #[test]
    fn setup_form_collects_credentials_without_rendering_the_password() {
        let device = record("192.0.2.10", "Example", "Camera");
        let mut form = SetupForm::new(device, "camera-192-0-2-10".to_owned());

        assert!(
            form.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_none()
        );
        form.paste("admin");
        assert!(
            form.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_none()
        );
        form.paste("secret");
        assert!(matches!(
            form.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(SetupIntent::Submit)
        ));
        assert_eq!(
            setup_field_line(
                "Password",
                &form.password,
                form.field,
                SetupField::Password,
                true
            ),
            "> Password   ••••••_"
        );

        let setup = form.finish().expect("valid setup form");
        assert_eq!(setup.id, "camera-192-0-2-10");
        assert_eq!(setup.username, "admin");
        assert_eq!(setup.password.expose_secret(), "secret");
    }

    #[test]
    fn setup_form_rejects_empty_fields_and_escape_cancels() {
        let device = record("192.0.2.10", "Example", "Camera");
        let mut form = SetupForm::new(device, String::new());
        form.field = SetupField::Password;

        assert!(
            form.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_none()
        );
        assert_eq!(form.field, SetupField::Id);
        assert_eq!(form.error.as_deref(), Some("Device ID must not be empty."));
        assert!(matches!(
            form.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(SetupIntent::Cancel)
        ));
    }

    fn record(address: &str, manufacturer: &str, model: &str) -> DiscoveryRecord {
        DiscoveryRecord {
            endpoint: format!("urn:uuid:{address}"),
            types: Vec::new(),
            scopes: Vec::new(),
            xaddrs: vec![format!("http://{address}/onvif/device_service")],
            manufacturer: Some(manufacturer.to_owned()),
            model: Some(model.to_owned()),
            firmware_version: None,
            serial_number: None,
        }
    }

    fn view(
        address: &str,
        manufacturer: &str,
        model: &str,
        registered_device_id: Option<&str>,
    ) -> DiscoveryDeviceView {
        DiscoveryDeviceView {
            record: record(address, manufacturer, model),
            registration_status: if registered_device_id.is_some() {
                DiscoveryRegistrationStatus::Saved
            } else {
                DiscoveryRegistrationStatus::New
            },
            registered_device_id: registered_device_id.map(str::to_owned),
        }
    }

    #[test]
    fn vim_navigation_moves_across_pages_without_wrapping() {
        let devices = (1..=25)
            .map(|index| view(&format!("192.0.2.{index}"), "Example", "Camera", None))
            .collect::<Vec<_>>();
        let mut state = BrowserState::new(&devices, 10, devices.len());

        state.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(state.selected, 10);
        state.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(state.selected, 9);
        state.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert_eq!(state.selected, 24);
        state.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(state.selected, 24);
    }

    #[test]
    fn live_filter_matches_identity_address_and_registration() {
        let devices = vec![
            view("192.0.2.10", "GeoVision", "Front Camera", None),
            view("192.0.2.20", "Example", "Rear Camera", Some("loading-dock")),
        ];
        let mut state = BrowserState::new(&devices, 10, devices.len());

        state.query = "geovision".to_owned();
        state.rebuild_filter();
        assert_eq!(state.filtered, vec![0]);
        state.query = "loading-dock".to_owned();
        state.rebuild_filter();
        assert_eq!(state.filtered, vec![1]);
        state.query = "192.0.2.2".to_owned();
        state.rebuild_filter();
        assert_eq!(state.filtered, vec![1]);
        state.query.clear();
        state.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        assert_eq!(state.filtered, vec![1]);
        state.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert_eq!(state.filtered, vec![0]);
        state.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(state.filtered, vec![0, 1]);
    }

    #[test]
    fn details_mode_exposes_complete_discovery_metadata_and_returns_to_list() {
        let mut device = view(
            "192.0.2.20",
            "GeoVision",
            "GV-TBL8810",
            Some("loading-dock"),
        );
        device.record.firmware_version = Some("V111".to_owned());
        device.record.serial_number = Some("SERIAL-20".to_owned());
        device.record.types = vec!["tds:Device".to_owned()];
        device.record.scopes = vec!["onvif://www.onvif.org/location/loading-dock".to_owned()];
        device
            .record
            .xaddrs
            .push("https://192.0.2.20/onvif/device_service".to_owned());
        let devices = vec![device];
        let mut state = BrowserState::new(&devices, 5, devices.len());

        state.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert!(state.showing_details);
        let rendered = discovery_detail_lines(&devices[0], 120).join("\n");
        for expected in [
            "Status: saved",
            "Saved as: loading-dock",
            "Manufacturer: GeoVision",
            "Model: GV-TBL8810",
            "Firmware: V111",
            "Serial: SERIAL-20",
            "tds:Device",
            "https://192.0.2.20/onvif/device_service",
            "onvif://www.onvif.org/location/loading-dock",
        ] {
            assert!(rendered.contains(expected), "missing {expected}");
        }

        state.detail_max_scroll = 10;
        state.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(state.detail_scroll, 1);
        state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!state.showing_details);
        assert_eq!(state.detail_scroll, 0);
    }

    #[test]
    fn registered_or_addressless_devices_cannot_be_added() {
        let mut devices = vec![view("192.0.2.10", "Example", "Camera", Some("front-door"))];
        let mut state = BrowserState::new(&devices, 10, devices.len());
        assert!(
            state
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_none()
        );

        devices[0] = view("192.0.2.10", "Example", "Camera", None);
        devices[0].record.xaddrs.clear();
        devices[0].registration_status = DiscoveryRegistrationStatus::Incomplete;
        let mut state = BrowserState::new(&devices, 10, devices.len());
        assert!(
            state
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .is_none()
        );
    }

    #[test]
    fn unicode_truncation_uses_display_columns() {
        assert_eq!(
            UnicodeWidthStr::width(truncate_to_width("攝影機-Front", 8).as_str()),
            8
        );
        assert_eq!(fit_cell("攝影機", 8), "攝影機  ");
    }
}
