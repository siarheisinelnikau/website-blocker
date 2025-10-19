use chrono::Utc;
use eframe::egui::{self, Color32, RichText};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};
use website_blocker::{
    model::{Group, Schedule, State, parse_patterns},
    store::{Change, Connection, Store},
    updater::{self, Update},
};

enum StateJob {
    Load,
    Apply(Change),
    Connection,
}
enum UpdateJob {
    Check,
    Install(Update),
}
enum Event {
    State(Result<State, String>),
    Connection(Connection),
    InstallResult(String),
    Update(Result<Option<Update>, String>),
    Progress(u64, Option<u64>),
    Install(Result<(), String>),
}

#[derive(PartialEq)]
enum Tab {
    Focus,
    Rules,
    Settings,
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
    locked: bool,
    phrase: String,
    connection: Connection,
    next_connection: Instant,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, store: Store) -> Self {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(242, 244, 237);
        visuals.selection.bg_fill = Color32::from_rgb(177, 213, 197);
        cc.egui_ctx.set_visuals(visuals);
        let (event_tx, events) = mpsc::sync_channel(32);
        let (jobs, state_rx) = mpsc::sync_channel(8);
        let (updates, update_rx) = mpsc::sync_channel(2);
        let ctx = cc.egui_ctx.clone();
        let state_store = store.clone();
        let state_events = event_tx.clone();
        std::thread::spawn(move || {
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
                        Event::State(state_store.load())
                    }
                    StateJob::Apply(change) => {
                        Event::State(state_store.apply(change, Utc::now().timestamp()))
                    }
                    StateJob::Connection => Event::Connection(state_store.connection()),
                };
                if state_events.send(event).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        let ctx = cc.egui_ctx.clone();
        let update_store = store.clone();
        std::thread::spawn(move || {
            while let Ok(job) = update_rx.recv() {
                let event = match job {
                    UpdateJob::Check => Event::Update(updater::check()),
                    UpdateJob::Install(update) => {
                        let result = updater::download(&update, &update_store, |done, total| {
                            let _ = event_tx.try_send(Event::Progress(done, total));
                            ctx.request_repaint();
                        })
                        .and_then(|path| updater::prepare_install(&path, &update_store));
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
            locked: false,
            phrase: String::new(),
            connection: Connection::default(),
            next_connection: Instant::now(),
        };
        app.submit(StateJob::Load);
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
    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::State(result) => {
                    self.busy = false;
                    match result {
                        Ok(state) => {
                            self.state = state;
                            self.ready = true;
                            self.message =
                                "Local settings ready. Browser changes apply after its next sync."
                                    .into();
                            if !self.state.groups.iter().any(|g| g.id == self.selected) {
                                self.selected = self.state.groups.first().map_or(0, |g| g.id);
                            }
                            self.load_editor();
                            if !self.startup_checked {
                                self.startup_checked = true;
                                if self.state.startup_updates {
                                    self.update_job(UpdateJob::Check);
                                }
                            }
                        }
                        Err(e) => self.message = e,
                    }
                }
                Event::Connection(connection) => self.connection = connection,
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
                        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(e) => self.update_message = format!("Update was not installed: {e}"),
                    }
                }
            }
        }
    }

    fn focus(&mut self, ui: &mut egui::Ui) {
        let now = Utc::now().timestamp();
        ui.heading("Give your next task some space.");
        ui.label("Choose a website list, set a timer, and begin when you are ready.");
        ui.add_space(18.0);
        if let Some(focus) = &self.state.focus
            && focus.end > now
        {
            let remaining = focus.end - now;
            let name = self
                .state
                .groups
                .iter()
                .find(|g| g.id == focus.group_id)
                .map_or("Rule set", |g| &g.name);
            ui.label(
                RichText::new(format!("{:02}:{:02}", remaining / 60, remaining % 60))
                    .size(54.0)
                    .color(Color32::from_rgb(33, 92, 82)),
            );
            ui.label(format!(
                "{name} · {}",
                if focus.locked {
                    "Locked until the timer ends"
                } else {
                    "Flexible focus"
                }
            ));
        } else {
            ui.label(RichText::new("Ready when you are.").size(30.0));
            let previous = self.selected;
            egui::ComboBox::from_label("Website list")
                .selected_text(
                    self.state
                        .groups
                        .iter()
                        .find(|g| g.id == self.selected)
                        .map_or("Choose a list", |g| &g.name),
                )
                .show_ui(ui, |ui| {
                    for group in self.state.groups.iter().filter(|g| g.enabled) {
                        ui.selectable_value(&mut self.selected, group.id, &group.name);
                    }
                });
            if self.selected != previous {
                self.load_editor();
            }
            ui.horizontal(|ui| {
                ui.label("Minutes");
                ui.add(egui::DragValue::new(&mut self.minutes).range(1..=240));
            });
            ui.checkbox(&mut self.locked, "Lock this session until the timer ends");
            ui.label(
                "Locked sessions prevent rule edits, early stopping and breaks inside this app.",
            );
            if ui.button("Start focus session").clicked() {
                self.submit(StateJob::Apply(Change::Start {
                    group_id: self.selected,
                    minutes: self.minutes,
                    locked: self.locked,
                }));
            }
        }
        ui.add_space(18.0);
        ui.separator();
        ui.heading("Intentional breaks");
        if self.state.pause_until > now {
            ui.label(format!(
                "All rules paused for {} more seconds.",
                self.state.pause_until - now
            ));
            if ui.button("Resume blocking now").clicked() {
                self.submit(StateJob::Apply(Change::Resume));
            }
        }
        ui.label("Type TAKE A BREAK for a five-minute pause (once every 30 minutes), or END FOCUS to end a flexible session. Schedules resume independently.");
        ui.text_edit_singleline(&mut self.phrase);
        ui.add_enabled_ui(!self.state.locked(now), |ui| {
            ui.horizontal(|ui| {
                if ui.button("Take a 5-minute break").clicked() {
                    self.submit(StateJob::Apply(Change::Break(self.phrase.clone())));
                    self.phrase.clear();
                }
                if ui.button("End focus session").clicked() {
                    self.submit(StateJob::Apply(Change::Stop(self.phrase.clone())));
                    self.phrase.clear();
                }
            });
        });
        ui.add_space(16.0);
        ui.label("Closing this window does not stop browser schedules. WebsiteBlocker never changes hosts, DNS, firewall rules or other applications.");
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
        ui.heading("Websites & schedules");
        ui.label("Lists are active during their schedules or a focus session. With no schedule, a list is timer-only.");
        if self.state.locked(Utc::now().timestamp()) {
            ui.colored_label(
                Color32::DARK_RED,
                "Rule editing is locked until your focus timer ends.",
            );
        }
        ui.add_enabled_ui(!self.state.locked(Utc::now().timestamp()), |ui| {
            let previous = self.selected;
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("rule-list").selected_text(self.state.groups.iter().find(|g| g.id == self.selected).map_or("New rule set", |g| &g.name))
                    .show_ui(ui, |ui| { for g in &self.state.groups { ui.selectable_value(&mut self.selected, g.id, &g.name); } });
                if ui.add_enabled(self.state.groups.len() < 20, egui::Button::new("New list")).clicked() {
                    self.selected = self.state.groups.iter().map(|g| g.id).max().unwrap_or(0).saturating_add(1);
                    self.name = "New focus list".into(); self.enabled = true; self.patterns.clear(); self.schedules.clear();
                } else if previous != self.selected { self.load_editor(); }
            });
            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut self.name); ui.checkbox(&mut self.enabled, "Enabled"); });
            ui.label("One website per line: example.com includes subdomains; =example.com matches only that host. Add /path for a case-sensitive path prefix.");
            ui.add(egui::TextEdit::multiline(&mut self.patterns).desired_rows(5).desired_width(f32::INFINITY).hint_text("example.com\n=video.example.com/shorts"));
            ui.add_space(10.0); ui.label(RichText::new("Recurring schedules").strong());
            let mut remove = None;
            for (index, s) in self.schedules.iter_mut().enumerate() {
                ui.group(|ui| { ui.push_id(index, |ui| {
                    ui.horizontal(|ui| { for (day, label) in ["M", "Tu", "W", "Th", "F", "Sa", "Su"].iter().enumerate() { ui.checkbox(&mut s.days[day], *label); }
                        if ui.small_button("Remove").clicked() { remove = Some(index); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("From"); ui.add(egui::TextEdit::singleline(&mut s.start).desired_width(55.0));
                        ui.label("to"); ui.add(egui::TextEdit::singleline(&mut s.end).desired_width(55.0));
                        ui.label("Timezone"); ui.add(egui::TextEdit::singleline(&mut s.timezone).desired_width(170.0));
                    });
                }); });
            }
            if let Some(index) = remove { self.schedules.remove(index); }
            if ui.add_enabled(self.schedules.len() < 14, egui::Button::new("Add schedule")).clicked() { self.schedules.push(ScheduleDraft::default()); }
            ui.small("Times use the named IANA zone, e.g. Europe/Minsk, Europe/London or America/New_York. The browser validates the zone. Overnight ranges belong to their start day. DST repeats both occurrences and skips nonexistent times.");
            ui.add_space(10.0); ui.label(RichText::new("Global allowed websites").strong());
            ui.label("These exceptions take priority over every active list. The same domain/path syntax applies.");
            ui.add(egui::TextEdit::multiline(&mut self.allowlist).desired_rows(3).desired_width(f32::INFINITY).hint_text("help.example.com"));
            ui.horizontal(|ui| {
                if ui.button("Save websites & schedules").clicked() && let Err(e) = self.save_rules(false) { self.message = e; }
                if ui.button("Delete selected list").clicked() && let Err(e) = self.save_rules(true) { self.message = e; }
                if ui.button("Discard edits / reload").clicked() { self.submit(StateJob::Load); }
            });
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings & connection");
        ui.label(format!(
            "Installed version: {} · Windows x64 release target",
            updater::VERSION
        ));
        ui.add_space(10.0);
        ui.label(RichText::new("Browser companion").strong());
        if self.connection.at > 0 {
            ui.label(format!(
                "Native host last contacted {} seconds ago (revision {}).",
                (Utc::now().timestamp() - self.connection.at).max(0),
                self.connection.revision
            ));
        } else {
            ui.label("No browser handshake recorded yet.");
        }
        ui.label("A handshake confirms communication, not that a browser applied every rule. Check the extension popup for enforcement status and errors.");
        ui.label("1. Extract the companion ZIP and load extension/ as an unpacked Chromium extension.\n2. Copy its ID and run scripts/Register-NativeHost.ps1 with your EXE path, extension ID and browser.\n3. Click Sync with desktop in the extension popup.");
        ui.label("Setup is per Windows user and per browser. See docs/BROWSER-SETUP.md. This app never installs extensions automatically.");
        ui.add_space(12.0);
        ui.separator();
        ui.heading("Updates");
        let mut startup = self.state.startup_updates;
        if ui
            .checkbox(&mut startup, "Check for updates on startup")
            .changed()
        {
            self.submit(StateJob::Apply(Change::StartupUpdates(startup)));
        }
        ui.label("Startup checks are quiet and run in the background. Disabling them makes no startup update request. Manual checks stay available.");
        if ui
            .add_enabled(!self.update_busy, egui::Button::new("Check for updates"))
            .clicked()
        {
            self.update_job(UpdateJob::Check);
        }
        ui.label(&self.update_message);
        if !self.install_result.is_empty() {
            ui.label(format!("Last installation result: {}", self.install_result));
        }
        if let Some(update) = self.available.clone() {
            ui.label(format!("Version {} is available. Installation will close and restart this app; local settings are preserved.", update.version));
            if ui
                .add_enabled(!self.update_busy, egui::Button::new("Download and install"))
                .clicked()
            {
                self.update_job(UpdateJob::Install(update));
            }
        }
        if let Some((done, total)) = self.progress {
            if let Some(total) = total.filter(|v| *v > 0) {
                ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage());
            }
            ui.label(format!("Downloaded {} KiB", done / 1024));
        }
        ui.small("Updates use public GitHub HTTPS without login. A missing repository/release or network error is reported as unavailable. No update has been published by this project yet.");
        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new("Local data & scope").strong());
        ui.label(format!("Settings directory: {}", self.store_dir.display()));
        ui.label("No browsing history or page content is collected. Cached rules survive desktop closure for up to eight days without a successful native sync. An expired cache clears blocking and reports a connection error.");
        ui.label("This is a voluntary focus tool. The machine owner can disable the extension, edit local state, change the clock or use another browser. It does not enforce system-wide or uninstall-proof restrictions.");
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive(ctx);
        if self.next_connection <= Instant::now() {
            let _ = self.jobs.try_send(StateJob::Connection);
            self.next_connection = Instant::now() + Duration::from_secs(30);
        }
        // Timed repaint only while a visible countdown needs it; never a continuous animation.
        let now = Utc::now().timestamp();
        let countdown =
            self.state.focus.as_ref().is_some_and(|f| f.end > now) || self.state.pause_until > now;
        ctx.request_repaint_after(if countdown {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(30)
        });
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("◎  WebsiteBlocker")
                        .size(23.0)
                        .strong()
                        .color(Color32::from_rgb(33, 92, 82)),
                );
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Focus, "Focus");
                ui.selectable_value(&mut self.tab, Tab::Rules, "Websites & schedules");
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
            });
            ui.add_space(8.0);
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
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(14.0);
                ui.add_enabled_ui(self.ready && !self.busy, |ui| match self.tab {
                    Tab::Focus => self.focus(ui),
                    Tab::Rules => self.rules(ui),
                    Tab::Settings => self.settings(ui),
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn time_input_bounds() {
        assert_eq!(parse_time("23:59").unwrap(), 1439);
        for value in ["24:00", "12:60", "noon", "-1:00"] {
            assert!(parse_time(value).is_err());
        }
    }
}
