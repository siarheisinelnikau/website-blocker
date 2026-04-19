use chrono::Utc;
const ACCENT: Color32 = Color32::from_rgb(25, 100, 73);
fn primary(ui: &mut egui::Ui, label: impl Into<String>) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label.into()).color(Color32::WHITE))
            .fill(ACCENT)
            .min_size(egui::vec2(180.0, 40.0)),
    )
}
use egui::{self, Color32, RichText};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};
use website_blocker::{
    model::{Group, Schedule, State, parse_patterns},
    setup::{self, Browser, Overview, UserRegistry},
    store::{Change, Store},
    updater::{self, Update},
};

enum StateJob {
    Load,
    Apply(Change),
    SetupScan(bool),
    PrepareBrowser(Browser),
    OpenBrowser(Browser),
    CompanionFolder,
}
enum UpdateJob {
    Check,
    Install(Box<Update>),
}
enum Event {
    State(Result<State, String>, bool),
    InstallResult(String),
    Update(Result<Option<Update>, String>),
    Progress(u64, Option<u64>),
    Install(Result<(), String>),
    Setup(Overview),
    SetupError(String),
    Prepared(Result<Overview, String>),
    BrowserOpened(Result<(), String>),
    CompanionFolder(Result<String, String>),
}

#[derive(PartialEq)]
enum Tab {
    Focus,
    Rules,
    Settings,
    Browser,
}

#[derive(Clone)]
struct ScheduleDraft {
    days: [bool; 7],
    start: String,
    end: String,
    timezone: String,
}
impl Default for ScheduleDraft {
    fn default() -> Self {
        Self {
            days: [true, true, true, true, true, false, false],
            start: "09:00".into(),
            end: "17:00".into(),
            timezone: "Europe/Minsk".into(),
        }
    }
}
impl ScheduleDraft {
    fn from_schedule(s: &Schedule) -> Self {
        Self {
            days: std::array::from_fn(|d| s.days.contains(&(d as u8))),
            start: time_text(s.start_minute),
            end: time_text(s.end_minute),
            timezone: s.timezone.clone(),
        }
    }
    fn parse(&self) -> Result<Schedule, String> {
        let value = Schedule {
            days: self
                .days
                .iter()
                .enumerate()
                .filter_map(|(day, enabled)| enabled.then_some(day as u8))
                .collect(),
            start_minute: parse_time(&self.start)?,
            end_minute: parse_time(&self.end)?,
            timezone: self.timezone.trim().into(),
        };
        value.validate()?;
        Ok(value)
    }
}
fn time_text(value: u16) -> String {
    format!("{:02}:{:02}", value / 60, value % 60)
}
fn parse_time(value: &str) -> Result<u16, String> {
    let (h, m) = value.trim().split_once(':').ok_or("Use HH:MM for times.")?;
    let h = h.parse::<u16>().map_err(|_| "Invalid hour.")?;
    let m = m.parse::<u16>().map_err(|_| "Invalid minute.")?;
    if h > 23 || m > 59 {
        return Err("Use a 24-hour time between 00:00 and 23:59.".into());
    }
    Ok(h * 60 + m)
}

pub struct App {
    store_dir: PathBuf,
    jobs: SyncSender<StateJob>,
    updates: SyncSender<UpdateJob>,
    events: Receiver<Event>,
    state: State,
    ready: bool,
    busy: bool,
    update_busy: bool,
    startup_checked: bool,
    tab: Tab,
    message: String,
    update_message: String,
    install_result: String,
    available: Option<Update>,
    progress: Option<(u64, Option<u64>)>,
    selected: u32,
    name: String,
    enabled: bool,
    patterns: String,
    allowlist: String,
    schedules: Vec<ScheduleDraft>,
    minutes: u32,
    custom_duration: bool,
    locked: bool,
    phrase: String,
    next_connection: Instant,
    setup: Overview,
    setup_loaded: bool,
    setup_working: bool,
    setup_browser: Browser,
    setup_message: String,
    setup_wait_until: Option<Instant>,
    companion_folder: Option<String>,
    pub close_requested: bool,
    pub diagnostic: bool,
}

impl App {
    pub fn with_context(ui_context: &egui::Context, store: Store) -> Self {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(244, 247, 244);
        visuals.override_text_color = Some(Color32::from_rgb(28, 54, 45));
        visuals.faint_bg_color = Color32::from_rgb(235, 241, 236);
        visuals.extreme_bg_color = Color32::WHITE;
        visuals.selection.bg_fill = Color32::from_rgb(201, 229, 214);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(230, 237, 232);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(230, 237, 232);
        visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(28, 54, 45);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(211, 229, 217);
        visuals.widgets.active.bg_fill = Color32::from_rgb(189, 216, 200);
        ui_context.set_visuals(visuals);
        ui_context.style_mut(|style| {
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(13.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(25.0));
            style.spacing.item_spacing = egui::vec2(12.0, 8.0);
            style.spacing.button_padding = egui::vec2(14.0, 8.0);
            style.spacing.interact_size.y = 34.0;
            style.animation_time = 0.0;
        });
        let (event_tx, events) = mpsc::sync_channel(32);
        let (jobs, state_rx) = mpsc::sync_channel(8);
        let (updates, update_rx) = mpsc::sync_channel(2);
        let ctx = ui_context.clone();
        let state_store = store.clone();
        let state_events = event_tx.clone();
        std::thread::spawn(move || {
            let mut last_setup: Option<Result<Overview, String>> = None;
            while let Ok(job) = state_rx.recv() {
                let event = match job {
                    StateJob::Load => {
                        if let Ok(bytes) = website_blocker::store::read_limited(
                            &state_store.dir.join("updates/install-result.txt"),
                            8192,
                        ) {
                            let _ = state_events
                                .send(Event::InstallResult(String::from_utf8_lossy(&bytes).into()));
                        }
                        Event::State(state_store.load(), true)
                    }
                    StateJob::Apply(change) => {
                        let reload_editor = matches!(&change, Change::Rules { .. });
                        Event::State(
                            state_store.apply(change, Utc::now().timestamp()),
                            reload_editor,
                        )
                    }
                    StateJob::SetupScan(force) => {
                        let result =
                            setup::inspect(&state_store, &UserRegistry, Utc::now().timestamp());
                        if !force && last_setup.as_ref() == Some(&result) {
                            continue;
                        }
                        last_setup = Some(result.clone());
                        match result {
                            Ok(overview) => Event::Setup(overview),
                            Err(error) => Event::SetupError(error),
                        }
                    }
                    StateJob::PrepareBrowser(browser) => {
                        let result = std::env::current_exe()
                            .map_err(|e| e.to_string())
                            .and_then(|exe| {
                                setup::prepare(&state_store, browser, &exe, &UserRegistry)
                            })
                            .and_then(|_| {
                                setup::inspect(&state_store, &UserRegistry, Utc::now().timestamp())
                            });
                        Event::Prepared(result)
                    }
                    StateJob::OpenBrowser(browser) => {
                        Event::BrowserOpened(setup::open_extensions(browser))
                    }
                    StateJob::CompanionFolder => {
                        Event::CompanionFolder(setup::companion_folder_text(&state_store))
                    }
                };
                if state_events.send(event).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        let ctx = ui_context.clone();
        std::thread::spawn(move || {
            while let Ok(job) = update_rx.recv() {
                let event = match job {
                    UpdateJob::Check => Event::Update(updater::check()),
                    UpdateJob::Install(update) => {
                        let result = updater::download(&update, |done, total| {
                            let _ = event_tx.try_send(Event::Progress(done, total));
                            ctx.request_repaint();
                        })
                        .and_then(updater::prepare_install);
                        Event::Install(result)
                    }
                };
                if event_tx.send(event).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        let mut app = Self {
            store_dir: store.dir,
            jobs,
            updates,
            events,
            state: State::default(),
            ready: false,
            busy: false,
            update_busy: false,
            startup_checked: false,
            tab: Tab::Focus,
            message: "Loading local settings…".into(),
            update_message: String::new(),
            install_result: String::new(),
            available: None,
            progress: None,
            selected: 1,
            name: String::new(),
            enabled: true,
            patterns: String::new(),
            allowlist: String::new(),
            schedules: vec![],
            minutes: 25,
            custom_duration: false,
            locked: false,
            phrase: String::new(),
            next_connection: Instant::now(),
            setup: Overview::default(),
            setup_loaded: false,
            setup_working: false,
            setup_browser: Browser::Chrome,
            setup_message: String::new(),
            setup_wait_until: None,
            companion_folder: None,
            close_requested: false,
            diagnostic: std::env::var_os("WEBSITEBLOCKER_CAPTURE").is_some()
                || std::env::var_os("WEBSITEBLOCKER_DIAGNOSTICS").is_some(),
        };
        app.submit(StateJob::Load);
        if let Ok(value) = std::env::var("WEBSITEBLOCKER_CAPTURE")
            && let Some((tab, path)) = value.split_once('|')
        {
            app.tab = match tab {
                "settings" => Tab::Settings,
                "rules" => Tab::Rules,
                "browser" => Tab::Browser,
                _ => Tab::Focus,
            };
            let _ = path;
        }
        app
    }

    fn submit(&mut self, job: StateJob) {
        match self.jobs.try_send(job) {
            Ok(()) => self.busy = true,
            Err(_) => {
                self.message = "The settings worker is busy or unavailable. Retry shortly.".into()
            }
        }
    }
    fn update_job(&mut self, job: UpdateJob) {
        match self.updates.try_send(job) {
            Ok(()) => {
                self.update_busy = true;
                self.update_message = "Working in the background…".into();
            }
            Err(_) => self.update_message = "The update worker is busy or unavailable.".into(),
        }
    }
    fn load_editor(&mut self) {
        if let Some(g) = self.state.groups.iter().find(|g| g.id == self.selected) {
            self.name = g.name.clone();
            self.enabled = g.enabled;
            self.patterns = g
                .patterns
                .iter()
                .map(|p| p.text())
                .collect::<Vec<_>>()
                .join("\n");
            self.schedules = g
                .schedules
                .iter()
                .map(ScheduleDraft::from_schedule)
                .collect();
        }
        self.allowlist = self
            .state
            .allowlist
            .iter()
            .map(|p| p.text())
            .collect::<Vec<_>>()
            .join("\n");
    }
    pub fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::State(result, mut reload_editor) => {
                    self.busy = false;
                    match result {
                        Ok(state) => {
                            if state.revision != self.state.revision {
                                self.setup.chrome.up_to_date = false;
                                self.setup.edge.up_to_date = false;
                                self.next_connection = Instant::now();
                            }
                            self.state = state;
                            self.ready = true;
                            self.message = "Saved on this computer.".into();
                            if !self.state.groups.iter().any(|g| g.id == self.selected) {
                                self.selected = self.state.groups.first().map_or(0, |g| g.id);
                                reload_editor = true;
                            }
                            if reload_editor {
                                self.load_editor();
                            }
                            if !self.startup_checked {
                                self.startup_checked = true;
                                if self.state.startup_updates && !self.diagnostic {
                                    self.update_job(UpdateJob::Check);
                                }
                            }
                        }
                        Err(e) => self.message = e,
                    }
                }
                Event::Setup(overview) => {
                    if !self.setup_loaded && !overview.chrome.available && overview.edge.available {
                        self.setup_browser = Browser::Edge;
                    }
                    self.setup_loaded = true;
                    self.setup = overview;
                    if self.setup.get(self.setup_browser).connected {
                        self.setup_wait_until = None;
                    }
                }
                Event::SetupError(error) => {
                    self.setup_loaded = true;
                    self.setup_message = error;
                }
                Event::CompanionFolder(result) => {
                    self.setup_working = false;
                    match result {
                        Ok(path) => {
                            ctx.copy_text(path.clone());
                            self.companion_folder = Some(path);
                            self.setup_message = "Folder path sent to the clipboard. In Load unpacked, paste it into the address bar (Ctrl+L), press Enter, then choose Select folder.".into();
                        }
                        Err(error) => self.setup_message = error,
                    }
                }
                Event::Prepared(result) => {
                    self.setup_working = false;
                    match result {
                        Ok(overview) => {
                            self.setup = overview;
                            self.setup_loaded = true;
                            self.setup_message = "Desktop connection prepared. Add the companion in your browser below.".into();
                            self.setup_wait_until = Some(Instant::now() + Duration::from_secs(120));
                        }
                        Err(error) => self.setup_message = error,
                    }
                }
                Event::BrowserOpened(result) => {
                    self.setup_working = false;
                    match result {
                        Ok(()) => {
                            self.setup_message = "Complete the browser step, then return here. Connection is checked automatically.".into();
                            self.setup_wait_until = Some(Instant::now() + Duration::from_secs(120));
                        }
                        Err(error) => self.setup_message = error,
                    }
                }
                Event::InstallResult(result) => self.install_result = result,
                Event::Update(result) => {
                    self.update_busy = false;
                    match result {
                        Ok(Some(update)) => {
                            self.update_message =
                                format!("Version {} is available.", update.version);
                            self.available = Some(update);
                        }
                        Ok(None) => {
                            self.available = None;
                            self.update_message = "You are up to date.".into();
                        }
                        Err(e) => self.update_message = e,
                    }
                }
                Event::Progress(done, total) => self.progress = Some((done, total)),
                Event::Install(result) => {
                    self.update_busy = false;
                    self.progress = None;
                    match result {
                        Ok(()) => self.close_requested = true,
                        Err(e) => self.update_message = format!("Update was not installed: {e}"),
                    }
                }
            }
        }
    }

    fn focus(&mut self, ui: &mut egui::Ui) {
        let now = Utc::now().timestamp();
        let active = self.state.focus.as_ref().is_some_and(|f| f.end > now);
        ui.heading(if active {
            "Your focus session"
        } else {
            "Ready to focus?"
        });
        if active && self.setup_loaded && !self.setup.connected() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Your current browser rules have not been confirmed yet.");
                if active && ui.button("Connect browser").clicked() {
                    self.tab = Tab::Browser;
                }
            });
        }
        ui.add_space(6.0);
        egui::Frame::new()
            .fill(Color32::WHITE)
            .corner_radius(12)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if active {
                    ui.label(
                        RichText::new("FOCUS IN PROGRESS")
                            .size(12.0)
                            .strong()
                            .color(ACCENT),
                    );
                }
                if let Some(focus) = &self.state.focus
                    && focus.end > now
                {
                    let remaining = focus.end - now;
                    ui.label(
                        RichText::new(format!("{:02}:{:02}", remaining / 60, remaining % 60))
                            .size(52.0)
                            .color(ACCENT),
                    );
                    let name = self
                        .state
                        .groups
                        .iter()
                        .find(|g| g.id == focus.group_id)
                        .map_or("Website list", |g| &g.name);
                    ui.label(format!(
                        "{name} · {}",
                        if focus.locked {
                            "Locked until the timer ends"
                        } else {
                            "Flexible session"
                        }
                    ));
                } else {
                    let previous = self.selected;
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.label("Website list");
                            egui::ComboBox::from_id_salt("focus-list")
                                .width(260.0)
                                .selected_text(
                                    self.state
                                        .groups
                                        .iter()
                                        .find(|g| g.id == self.selected)
                                        .map_or("Choose a list", |g| &g.name),
                                )
                                .show_ui(ui, |ui| {
                                    for group in self.state.groups.iter().filter(|g| g.enabled) {
                                        ui.selectable_value(
                                            &mut self.selected,
                                            group.id,
                                            &group.name,
                                        );
                                    }
                                });
                        });
                        ui.vertical(|ui| {
                            ui.label("Duration");
                            egui::ComboBox::from_id_salt("duration")
                                .width(160.0)
                                .selected_text(if self.custom_duration {
                                    "Custom duration".to_string()
                                } else {
                                    format!("{} minutes", self.minutes)
                                })
                                .show_ui(ui, |ui| {
                                    for minutes in [25, 50, 90] {
                                        if ui
                                            .selectable_label(
                                                !self.custom_duration && self.minutes == minutes,
                                                format!("{minutes} minutes"),
                                            )
                                            .clicked()
                                        {
                                            self.minutes = minutes;
                                            self.custom_duration = false;
                                        }
                                    }
                                    ui.selectable_value(
                                        &mut self.custom_duration,
                                        true,
                                        "Custom duration",
                                    );
                                });
                            if self.custom_duration {
                                ui.add(
                                    egui::DragValue::new(&mut self.minutes)
                                        .range(1..=240)
                                        .suffix(" min"),
                                );
                            }
                        });
                    });
                    if previous != self.selected {
                        self.load_editor();
                    }
                    ui.add_space(4.0);
                    ui.checkbox(&mut self.locked, "Lock session: no early stop or breaks");
                    ui.add_space(4.0);
                    if self.setup.connected() {
                        if primary(ui, format!("Start {}-minute focus", self.minutes)).clicked() {
                            self.submit(StateJob::Apply(Change::Start {
                                group_id: self.selected,
                                minutes: self.minutes,
                                locked: self.locked,
                            }));
                        }
                    } else if primary(ui, "Connect browser to start").clicked() {
                        self.tab = Tab::Browser;
                        self.next_connection = Instant::now();
                    }
                }
            });
        ui.add_space(10.0);
        if self.state.pause_until > now {
            ui.label(format!(
                "Blocking paused · {} seconds remaining",
                self.state.pause_until - now
            ));
            if ui.button("Resume blocking").clicked() {
                self.submit(StateJob::Apply(Change::Resume));
            }
        }
        egui::CollapsingHeader::new("Take a break or end a session").default_open(active).show(ui, |ui| {
            if self.state.locked(now) { ui.label("This session is locked. Blocking ends when its timer runs out."); return; }
            ui.small("Type TAKE A BREAK for a five-minute pause, or END FOCUS to stop a flexible session.");
            ui.add(egui::TextEdit::singleline(&mut self.phrase).margin(egui::Margin::symmetric(8, 8)).desired_width(300.0).hint_text("Confirmation phrase"));
            ui.horizontal_wrapped(|ui| {
                if ui.button("Take 5-minute break").clicked() { self.submit(StateJob::Apply(Change::Break(self.phrase.clone()))); self.phrase.clear(); }
                if ui.add_enabled(active, egui::Button::new("End session")).clicked() { self.submit(StateJob::Apply(Change::Stop(self.phrase.clone()))); self.phrase.clear(); }
            });
            ui.small("One break per 30 minutes. Scheduled blocks continue after a focus session ends.");
        });
        ui.add_space(8.0);
        ui.small("Schedules keep running when this window is closed.");
    }

    fn save_rules(&mut self, delete: bool) -> Result<(), String> {
        let mut groups = self.state.groups.clone();
        groups.retain(|g| g.id != self.selected);
        if !delete {
            groups.push(Group {
                id: self.selected,
                name: self.name.trim().into(),
                enabled: self.enabled,
                patterns: parse_patterns(&self.patterns)?,
                schedules: self
                    .schedules
                    .iter()
                    .map(ScheduleDraft::parse)
                    .collect::<Result<_, _>>()?,
            });
        }
        groups.sort_by_key(|g| g.id);
        let allowlist = parse_patterns(&self.allowlist)?;
        let mut candidate = self.state.clone();
        candidate.groups = groups.clone();
        candidate.allowlist = allowlist.clone();
        // Expired sessions no longer retain deleted rule sets.
        if candidate
            .focus
            .as_ref()
            .is_some_and(|f| f.end <= Utc::now().timestamp())
        {
            candidate.focus = None;
        }
        candidate.validate()?;
        self.submit(StateJob::Apply(Change::Rules {
            revision: self.state.revision,
            groups,
            allowlist,
        }));
        Ok(())
    }

    fn rules(&mut self, ui: &mut egui::Ui) {
        ui.heading("Websites");
        if self.state.locked(Utc::now().timestamp()) {
            ui.colored_label(
                Color32::DARK_RED,
                "Editing is locked until your focus timer ends.",
            );
        }
        ui.add_enabled_ui(!self.state.locked(Utc::now().timestamp()), |ui| {
            let previous = self.selected;
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("rule-list").width(230.0)
                    .selected_text(self.state.groups.iter().find(|g| g.id == self.selected).map_or("New website list", |g| &g.name))
                    .show_ui(ui, |ui| { for g in &self.state.groups { ui.selectable_value(&mut self.selected, g.id, &g.name); } });
                if ui.add_enabled(self.state.groups.len() < 20, egui::Button::new("New list")).clicked() {
                    self.selected = self.state.groups.iter().map(|g| g.id).max().unwrap_or(0).saturating_add(1);
                    self.name = "New focus list".into(); self.enabled = true; self.patterns.clear(); self.schedules.clear();
                } else if previous != self.selected { self.load_editor(); }
            });
            ui.horizontal(|ui| { ui.label("Name"); ui.add(egui::TextEdit::singleline(&mut self.name).margin(egui::Margin::symmetric(8, 8)).desired_width(270.0)); ui.checkbox(&mut self.enabled, "Enabled"); });
            ui.label(RichText::new("Websites to block").strong());
            ui.add(egui::TextEdit::multiline(&mut self.patterns).desired_rows(4).desired_width(f32::INFINITY).hint_text("example.com\nvideo.example.com/shorts"));
            egui::CollapsingHeader::new("Website matching help").show(ui, |ui| {
                ui.small("One website per line. example.com includes subdomains; =example.com is an exact host. /path is a case-sensitive prefix. Use domains without https://.");
            });
            egui::CollapsingHeader::new(format!("Weekly schedules ({})", self.schedules.len())).default_open(!self.schedules.is_empty()).show(ui, |ui| {
                let mut remove = None;
                for (index, schedule) in self.schedules.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (day, label) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].iter().enumerate() { ui.toggle_value(&mut schedule.days[day], *label); }
                            if ui.small_button("Remove").clicked() { remove = Some(index); }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut schedule.start).margin(egui::Margin::symmetric(8, 8)).desired_width(65.0)); ui.label("to");
                            ui.add(egui::TextEdit::singleline(&mut schedule.end).margin(egui::Margin::symmetric(8, 8)).desired_width(65.0)); ui.label("in");
                            ui.add(egui::TextEdit::singleline(&mut schedule.timezone).margin(egui::Margin::symmetric(8, 8)).desired_width(180.0));
                        });
                    });
                }
                if let Some(index) = remove { self.schedules.remove(index); }
                if ui.add_enabled(self.schedules.len() < 14, egui::Button::new("Add schedule")).clicked() { self.schedules.push(ScheduleDraft::default()); }
                ui.small("Use a named timezone, e.g. Europe/Minsk. Overnight ranges belong to their start day.");
            });
            egui::CollapsingHeader::new("Always allowed websites").default_open(!self.allowlist.is_empty()).show(ui, |ui| {
                ui.small("Exceptions take priority over every active list.");
                ui.add(egui::TextEdit::multiline(&mut self.allowlist).desired_rows(2).desired_width(f32::INFINITY).hint_text("help.example.com"));
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if primary(ui, "Save changes").clicked() && let Err(error) = self.save_rules(false) { self.message = error; }
                if ui.button("Discard edits").clicked() { self.submit(StateJob::Load); }
                ui.add_space(16.0);
                if ui.button(RichText::new("Delete list").color(Color32::from_rgb(150, 53, 43))).clicked() && let Err(error) = self.save_rules(true) { self.message = error; }
            });
        });
    }

    fn setup_job(&mut self, job: StateJob) {
        if self.jobs.try_send(job).is_ok() {
            self.setup_working = true;
        } else {
            self.setup_message = "The connection worker is busy. Try again shortly.".into();
        }
    }
    fn connection_card(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Browser connection").size(19.0).strong());
        let label = if self.setup.chrome.connected && self.setup.edge.connected {
            "Chrome and Edge are connected"
        } else if self.setup.chrome.connected {
            "Chrome is connected"
        } else if self.setup.edge.connected {
            "Edge is connected"
        } else if self.setup_loaded {
            "No browser connected yet"
        } else {
            "Checking browser connection…"
        };
        ui.label(label);
        if primary(
            ui,
            if self.setup.connected() {
                "Manage connection"
            } else {
                "Connect browser"
            },
        )
        .clicked()
        {
            self.tab = Tab::Browser;
            self.next_connection = Instant::now();
        }
    }
    fn browser_setup(&mut self, ui: &mut egui::Ui) {
        if ui.small_button("Back to Settings").clicked() {
            self.tab = Tab::Settings;
        }
        ui.heading("Connect your browser");
        ui.horizontal(|ui| {
            for browser in [Browser::Chrome, Browser::Edge] {
                if ui
                    .selectable_value(&mut self.setup_browser, browser, browser.label())
                    .clicked()
                {
                    self.setup_message.clear();
                    self.next_connection = Instant::now();
                }
            }
        });
        let browser = self.setup_browser;
        let status = self.setup.get(browser).clone();
        if status.connected {
            ui.add_space(12.0);
            ui.colored_label(
                ACCENT,
                if status.up_to_date {
                    format!(
                        "{} is connected. Your current website rules have been applied.",
                        browser.label()
                    )
                } else {
                    format!(
                        "{} is connected. Your recent changes are syncing.",
                        browser.label()
                    )
                },
            );
            ui.small("You can close this window. The browser keeps your schedules running.");
            if primary(ui, "Go to Focus").clicked() {
                self.tab = Tab::Focus;
            }
            if !self.setup_message.is_empty() {
                ui.label(&self.setup_message);
            }
            egui::CollapsingHeader::new("Repair or move the app").show(ui, |ui| {
                if ui
                    .add_enabled(
                        !self.setup_working,
                        egui::Button::new("Prepare connection again"),
                    )
                    .clicked()
                {
                    self.setup_job(StateJob::PrepareBrowser(browser));
                }
            });
            return;
        }
        if !status.available {
            ui.label(format!(
                "{} was not found. Install it, then choose Check connection.",
                browser.label()
            ));
        }
        ui.add_space(8.0);
        ui.label(RichText::new("1. Prepare the desktop connection").strong());
        if status.prepared {
            ui.colored_label(
                ACCENT,
                "Ready. The companion files and browser connection are prepared.",
            );
            if ui
                .add_enabled(
                    !self.setup_working,
                    egui::Button::new("Update connection files"),
                )
                .clicked()
            {
                self.setup_job(StateJob::PrepareBrowser(browser));
            }
        } else {
            ui.small("WebsiteBlocker prepares the included companion automatically.");
            if ui
                .add_enabled(
                    status.available && !self.setup_working,
                    egui::Button::new(
                        RichText::new(format!("Connect {}", browser.label())).color(Color32::WHITE),
                    )
                    .fill(ACCENT),
                )
                .clicked()
            {
                self.setup_job(StateJob::PrepareBrowser(browser));
                self.next_connection = Instant::now();
            }
        }
        if !status.prepared {
            ui.add_space(8.0);
            ui.small("Next: add the companion in your browser, then confirm the connection.");
            if !status.error.is_empty() {
                ui.colored_label(Color32::DARK_RED, &status.error);
            }
            if !self.setup_message.is_empty() {
                ui.label(&self.setup_message);
            }
            if self.setup_working {
                ui.label("Preparing connection…");
            }
            return;
        }
        ui.add_space(8.0);
        ui.label(RichText::new(format!("2. Add the companion in {}", browser.label())).strong());
        ui.small("Local preview: the companion is not in browser stores yet. This browser step is required once.");
        ui.add_enabled_ui(status.prepared && !self.setup_working, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(format!("Open {} extensions", browser.label()))
                    .clicked()
                {
                    self.setup_job(StateJob::OpenBrowser(browser));
                }
                if ui.button("Copy companion folder").clicked() {
                    self.setup_job(StateJob::CompanionFolder);
                }
            });
        });
        ui.label("In the browser: turn on Developer mode, choose Load unpacked, then select the copied companion folder. Already added? Choose Reload on its card.");
        if let Some(path) = &self.companion_folder {
            ui.add(egui::TextEdit::singleline(&mut path.as_str()).desired_width(f32::INFINITY));
        }
        egui::CollapsingHeader::new("Added an older preview before?").show(ui, |ui| {
            ui.small("Disable or remove the earlier WebsiteBlocker copy in the browser before adding this companion. Keep only one enabled copy in each profile.");
        });
        ui.add_space(8.0);
        ui.label(RichText::new("3. Verify the connection").strong());
        ui.small("The companion connects automatically after it is added. This page confirms when your rules are actually applied.");
        if !status.error.is_empty() {
            ui.colored_label(Color32::DARK_RED, &status.error);
        }
        if self.setup_working {
            ui.label("Preparing connection…");
        }
        if !self.setup_message.is_empty() {
            ui.label(&self.setup_message);
        }
        if ui
            .add_enabled(!self.setup_working, egui::Button::new("Check connection"))
            .clicked()
        {
            let _ = self.jobs.try_send(StateJob::SetupScan(true));
            self.setup_wait_until = Some(Instant::now() + Duration::from_secs(120));
            self.next_connection = Instant::now();
            self.setup_message =
                "Waiting for the browser companion. Complete step 2 if it has not been added yet."
                    .into();
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Settings");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(format!("WebsiteBlocker {}", updater::VERSION));
            });
        });
        ui.add_space(10.0);
        self.connection_card(ui);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        ui.label(RichText::new("Updates").size(19.0).strong());
        let mut startup = self.state.startup_updates;
        if ui
            .checkbox(&mut startup, "Check for updates on startup")
            .changed()
        {
            self.submit(StateJob::Apply(Change::StartupUpdates(startup)));
        }
        if ui
            .add_enabled(!self.update_busy, egui::Button::new("Check for updates"))
            .clicked()
        {
            self.update_job(UpdateJob::Check);
        }
        if !self.update_message.is_empty() {
            ui.label(&self.update_message);
        }
        if let Some(update) = self.available.clone() {
            ui.label(format!(
                "Version {} is available. Your settings will be preserved; the app will restart.",
                update.version
            ));
            if ui
                .add_enabled(
                    !self.update_busy,
                    egui::Button::new(RichText::new("Download and install").color(Color32::WHITE))
                        .fill(ACCENT),
                )
                .clicked()
            {
                self.update_job(UpdateJob::Install(Box::new(update)));
            }
        }
        if let Some((done, total)) = self.progress {
            if let Some(total) = total.filter(|value| *value > 0) {
                ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage());
            }
            ui.small(format!("Downloaded {} KiB", done / 1024));
        }
        if !self.install_result.is_empty() {
            ui.label(format!("Last installation: {}", self.install_result));
        }
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Local data & privacy").show(ui, |ui| {
            ui.label(format!("Settings: {}", self.store_dir.display()));
            ui.label("Browsing history and page content are not recorded. Update checks contact GitHub. Rules stay local.");
            ui.small("A valid cached policy runs for up to eight days without a desktop sync; expired rules clear automatically. This is a voluntary focus tool that the machine owner can bypass.");
        });
    }
}

impl App {
    pub fn capture_ready(&self) -> bool {
        self.ready && !self.busy && self.setup_loaded
    }
    pub fn maintenance_deadline(&self) -> Option<Instant> {
        Some(self.next_connection)
    }
    pub fn maintenance_tick(&mut self) {
        if self.next_connection <= Instant::now() {
            let _ = self.jobs.try_send(StateJob::SetupScan(false));
            let waiting = self.tab == Tab::Browser
                && self
                    .setup_wait_until
                    .is_some_and(|until| until > Instant::now());
            self.next_connection =
                Instant::now() + Duration::from_secs(if waiting { 1 } else { 30 });
        }
    }
    pub fn render(&mut self, ctx: &egui::Context) {
        self.receive(ctx);
        let now = Utc::now().timestamp();
        if self.tab == Tab::Focus
            && (self.state.focus.as_ref().is_some_and(|f| f.end > now)
                || self.state.pause_until > now)
        {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("◎  WebsiteBlocker")
                        .size(19.0)
                        .strong()
                        .color(Color32::from_rgb(33, 92, 82)),
                );
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Focus, "Focus");
                ui.selectable_value(&mut self.tab, Tab::Rules, "Websites");
                if ui
                    .selectable_label(matches!(self.tab, Tab::Settings | Tab::Browser), "Settings")
                    .clicked()
                {
                    self.tab = Tab::Settings;
                }
            });
            ui.add_space(4.0);
            if let Some(update) = &self.available {
                ui.label(format!(
                    "Update {} available — open Settings to install.",
                    update.version
                ));
            }
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.message);
            if self.busy {
                ui.small("Saving or loading…");
            }
            if !self.ready && ui.button("Retry loading settings").clicked() {
                self.submit(StateJob::Load);
            }
        });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(244, 247, 244))
                    .inner_margin(18.0),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(790.0));
                    ui.add_enabled_ui(self.ready && !self.busy, |ui| match self.tab {
                        Tab::Focus => self.focus(ui),
                        Tab::Rules => self.rules(ui),
                        Tab::Settings => self.settings(ui),
                        Tab::Browser => self.browser_setup(ui),
                    });
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle(app: &mut App, context: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.busy && Instant::now() < deadline {
            app.receive(context);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy, "{}", app.message);
        assert!(app.ready, "{}", app.message);
    }

    #[test]
    fn settings_and_focus_actions_preserve_unsaved_website_edits() {
        std::fs::create_dir_all(".test-output").unwrap();
        let temp = tempfile::tempdir_in(".test-output").unwrap();
        let store = Store {
            dir: temp.path().into(),
        };
        store
            .apply(Change::StartupUpdates(false), Utc::now().timestamp())
            .unwrap();
        let context = egui::Context::default();
        let mut app = App::with_context(&context, store);
        settle(&mut app, &context);
        app.name = "Unsaved list name".into();
        app.patterns = "work.example.com".into();
        app.allowlist = "help.example.com".into();
        app.submit(StateJob::Apply(Change::StartupUpdates(true)));
        settle(&mut app, &context);
        app.submit(StateJob::Apply(Change::Start {
            group_id: 1,
            minutes: 1,
            locked: false,
        }));
        settle(&mut app, &context);
        assert_eq!(app.name, "Unsaved list name");
        assert_eq!(app.patterns, "work.example.com");
        assert_eq!(app.allowlist, "help.example.com");
        app.submit(StateJob::Load);
        settle(&mut app, &context);
        assert_eq!(app.name, "Distractions");
        assert_ne!(app.patterns, "work.example.com");
    }

    #[test]
    fn time_input_bounds() {
        assert_eq!(parse_time("23:59").unwrap(), 1439);
        for value in ["24:00", "12:60", "noon", "-1:00"] {
            assert!(parse_time(value).is_err());
        }
    }
}
