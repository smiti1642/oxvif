use std::{
    future::Future,
    io::{self, Write},
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Print,
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
    BeginSetup(Box<DiscoveryRecord>),
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
        state.set_page_size(
            usize::from(height)
                .saturating_sub(8)
                .clamp(1, DEFAULT_PAGE_SIZE),
        );
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
                        BrowserIntent::BeginSetup(device) => {
                            let target = primary_target(&device).ok_or_else(|| {
                                AppError::invalid_argument(
                                    "The selected discovery record has no usable device-service address.",
                                )
                            })?;
                            let suggested_id = super::suggested_device_id(target, None)?;
                            setup_form = Some(SetupForm::new(*device, suggested_id));
                        }
                    }
                }
            }
            Event::Paste(value) => {
                if let Some(form) = setup_form.as_mut() {
                    form.paste(&value);
                } else if state.filtering {
                    state.query.push_str(&value);
                    state.rebuild_filter();
                }
            }
            _ => {}
        }
    }
}

struct TerminalSession {
    stdout: io::Stdout,
    previous_lines: Vec<String>,
}

impl TerminalSession {
    fn enter() -> Result<Self, AppError> {
        enable_raw_mode().map_err(terminal_error)?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(terminal_error(error));
        }
        Ok(Self {
            stdout,
            previous_lines: Vec::new(),
        })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
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
    devices: &'a [DiscoveryDeviceView],
    total_count: usize,
    filtered: Vec<usize>,
    selected: usize,
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
            devices,
            total_count,
            filtered: (0..devices.len()).collect(),
            selected: 0,
            page_size: page_size.max(1),
            query: String::new(),
            filtering: false,
            registration_view: RegistrationView::All,
            showing_details: false,
            detail_scroll: 0,
            detail_max_scroll: 0,
        }
    }

    fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
        self.clamp_selection();
    }

    fn rebuild_filter(&mut self) {
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
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.filtered.len() - 1);
        }
    }

    fn page_start(&self) -> usize {
        self.selected / self.page_size * self.page_size
    }

    fn page_count(&self) -> usize {
        self.filtered.len().max(1).div_ceil(self.page_size)
    }

    fn current_page(&self) -> usize {
        if self.filtered.is_empty() {
            1
        } else {
            self.selected / self.page_size + 1
        }
    }

    fn current(&self) -> Option<&'a DiscoveryDeviceView> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.devices.get(*index))
    }

    fn move_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
        }
    }

    fn move_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn next_page(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.page_start() + self.page_size).min(self.filtered.len() - 1);
        }
    }

    fn previous_page(&mut self) {
        self.selected = self.page_start().saturating_sub(self.page_size);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<BrowserIntent> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(BrowserIntent::Quit);
        }
        if self.showing_details {
            match key.code {
                KeyCode::Esc | KeyCode::Char('i') => {
                    self.showing_details = false;
                    self.detail_scroll = 0;
                }
                KeyCode::Char('q') => return Some(BrowserIntent::Quit),
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_scroll = (self.detail_scroll + 1).min(self.detail_max_scroll);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
                KeyCode::Right | KeyCode::PageDown | KeyCode::Char('l') => {
                    self.detail_scroll =
                        (self.detail_scroll + self.page_size).min(self.detail_max_scroll);
                }
                KeyCode::Left | KeyCode::PageUp | KeyCode::Char('h') => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(self.page_size);
                }
                KeyCode::Home | KeyCode::Char('g') => self.detail_scroll = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    self.detail_scroll = self.detail_max_scroll;
                }
                _ => {}
            }
            return None;
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

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(BrowserIntent::Quit),
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_next();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_previous();
                None
            }
            KeyCode::Right | KeyCode::PageDown | KeyCode::Char('l') => {
                self.next_page();
                None
            }
            KeyCode::Left | KeyCode::PageUp | KeyCode::Char('h') => {
                self.previous_page();
                None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
                None
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.filtered.is_empty() {
                    self.selected = self.filtered.len() - 1;
                }
                None
            }
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
            KeyCode::Enter | KeyCode::Char('a') => self.current().and_then(|device| {
                (device.registration_status != DiscoveryRegistrationStatus::Saved
                    && primary_target(device).is_some())
                .then(|| BrowserIntent::BeginSetup(Box::new(device.record.clone())))
            }),
            _ => None,
        }
    }
}

fn render_setup(terminal: &mut TerminalSession, form: &SetupForm) -> Result<(), AppError> {
    let (width, _) = terminal::size().map_err(terminal_error)?;
    let width = usize::from(width).max(40);
    let mut lines = Vec::with_capacity(13);
    let target = primary_target(&form.device).unwrap_or("(no usable address)");

    push_line(
        &mut lines,
        "oxvif discovery - onboard selected device",
        width,
    );
    push_line(
        &mut lines,
        &format!("Device: {}", primary_address(&form.device)),
        width,
    );
    push_line(&mut lines, &format!("Target: {target}"), width);
    push_line(
        &mut lines,
        &format!("Endpoint: {}", display_endpoint(&form.device)),
        width,
    );
    push_line(&mut lines, "", width);
    push_line(
        &mut lines,
        &setup_field_line("Device ID", &form.id, form.field, SetupField::Id, false),
        width,
    );
    push_line(
        &mut lines,
        &setup_field_line(
            "Username",
            &form.username,
            form.field,
            SetupField::Username,
            false,
        ),
        width,
    );
    push_line(
        &mut lines,
        &setup_field_line(
            "Password",
            &form.password,
            form.field,
            SetupField::Password,
            true,
        ),
        width,
    );
    push_line(&mut lines, "", width);
    push_line(
        &mut lines,
        form.error
            .as_deref()
            .unwrap_or("Nothing is saved until the form is submitted and setup succeeds."),
        width,
    );
    push_line(
        &mut lines,
        "Tab or Up/Down: field | Enter: next/submit | Ctrl-U: clear field | Esc: back",
        width,
    );
    draw_changed_lines(terminal, lines)
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

fn render(terminal: &mut TerminalSession, state: &mut BrowserState<'_>) -> Result<(), AppError> {
    if state.showing_details {
        return render_details(terminal, state);
    }
    let (width, _) = terminal::size().map_err(terminal_error)?;
    let width = usize::from(width).max(40);
    let mut lines = Vec::with_capacity(state.page_size + 7);
    let saved_count = state
        .devices
        .iter()
        .filter(|device| device.registration_status == DiscoveryRegistrationStatus::Saved)
        .count();
    let new_count = state
        .devices
        .iter()
        .filter(|device| device.registration_status == DiscoveryRegistrationStatus::New)
        .count();
    let incomplete_count = state.devices.len() - saved_count - new_count;

    push_line(
        &mut lines,
        &format!(
            "oxvif discovery - {} found | {} saved | {} new | {} incomplete",
            state.total_count, saved_count, new_count, incomplete_count
        ),
        width,
    );
    let filter = if state.query.is_empty() {
        "(none)"
    } else {
        state.query.as_str()
    };
    push_line(
        &mut lines,
        &format!(
            "View: {} | {} shown | page {}/{} | Search: {filter}{}",
            registration_view_name(state.registration_view),
            state.filtered.len(),
            state.current_page(),
            state.page_count(),
            if state.filtering { "_" } else { "" }
        ),
        width,
    );
    push_line(&mut lines, "", width);
    push_line(
        &mut lines,
        "  #    STATUS      ADDRESS              DEVICE                  SAVED AS",
        width,
    );

    let start = state.page_start();
    for position in start..(start + state.page_size) {
        let Some(device_index) = state.filtered.get(position) else {
            push_line(&mut lines, "", width);
            continue;
        };
        let device = &state.devices[*device_index];
        let marker = if position == state.selected { '>' } else { ' ' };
        let line = format!(
            "{marker} {:<4} {} {} {} {}",
            device_index + 1,
            fit_cell(
                &device.registration_status.as_str().to_ascii_uppercase(),
                10
            ),
            fit_cell(&primary_address(device), 20),
            fit_cell(&discovery_device_label(device), 23),
            fit_cell(device.registered_device_id.as_deref().unwrap_or("-"), 16),
        );
        push_line(&mut lines, &line, width);
    }

    push_line(&mut lines, "", width);
    if let Some(device) = state.current() {
        let detail = if let Some(id) = device.registered_device_id.as_deref() {
            format!("Already registered as {id} | {}", display_endpoint(device))
        } else if primary_target(device).is_none() {
            format!(
                "No usable address; add is unavailable | {}",
                display_endpoint(device)
            )
        } else {
            format!(
                "Enter/a: add {} | {}",
                primary_address(device),
                display_endpoint(device)
            )
        };
        push_line(&mut lines, &detail, width);
    } else {
        push_line(&mut lines, "No devices match the current filter.", width);
    }
    push_line(
        &mut lines,
        if state.filtering {
            "Type to filter | Enter/Esc: return | Ctrl-U: clear | Ctrl-C: quit"
        } else {
            "j/k: move | h/l: page | i: details | /: search | r: saved | n: unregistered | A: all | q: quit"
        },
        width,
    );
    draw_changed_lines(terminal, lines)
}

fn render_details(
    terminal: &mut TerminalSession,
    state: &mut BrowserState<'_>,
) -> Result<(), AppError> {
    let (width, _) = terminal::size().map_err(terminal_error)?;
    let width = usize::from(width).max(40);
    let Some(device) = state.current() else {
        state.showing_details = false;
        return render(terminal, state);
    };
    let content = discovery_detail_lines(device, width);
    state.detail_max_scroll = content.len().saturating_sub(state.page_size);
    state.detail_scroll = state.detail_scroll.min(state.detail_max_scroll);
    let start = state.detail_scroll;
    let end = (start + state.page_size).min(content.len());
    let mut lines = Vec::with_capacity(state.page_size + 5);

    push_line(&mut lines, "oxvif discovery - device details", width);
    push_line(
        &mut lines,
        &format!(
            "Record {} of {} | details {}-{} of {}",
            state.selected.saturating_add(1),
            state.filtered.len(),
            if content.is_empty() { 0 } else { start + 1 },
            end,
            content.len()
        ),
        width,
    );
    push_line(&mut lines, "", width);
    for line in content.iter().skip(start).take(state.page_size) {
        push_line(&mut lines, line, width);
    }
    for _ in end..(start + state.page_size) {
        push_line(&mut lines, "", width);
    }
    push_line(&mut lines, "", width);
    push_line(
        &mut lines,
        "j/k: scroll | h/l: page | g/G: first/last | i/Esc: back | q: quit",
        width,
    );
    draw_changed_lines(terminal, lines)
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

fn push_line(lines: &mut Vec<String>, value: &str, width: usize) {
    lines.push(truncate_to_width(value, width));
}

fn draw_changed_lines(terminal: &mut TerminalSession, lines: Vec<String>) -> Result<(), AppError> {
    queue!(terminal.stdout, BeginSynchronizedUpdate).map_err(terminal_error)?;
    let row_count = lines.len().max(terminal.previous_lines.len());
    for row in 0..row_count {
        let current = lines.get(row).map_or("", String::as_str);
        let previous = terminal.previous_lines.get(row).map_or("", String::as_str);
        if current != previous {
            queue!(
                terminal.stdout,
                MoveTo(0, u16::try_from(row).unwrap_or(u16::MAX)),
                Print(current),
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
    if UnicodeWidthStr::width(value) <= width {
        return value.to_owned();
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
