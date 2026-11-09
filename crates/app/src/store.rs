//! What the window and the tray show, and what they can do. There is one per app, alive whether
//! or not the window is open, so the tray stays current with the window closed.

use crate::i18n::{t, tf, tn};
use crate::model::{self, Account, Busy, Mood};
use crate::theme::ThemeChoice;
use cc_same_core::accounts::{self, Refused as ListRefused};
use cc_same_core::config::Config;
use cc_same_core::desktop::{NotReady, Quitting, Watch};
use cc_same_core::logins::{self, Refused};
use cc_same_core::report::{self, Overview};
use cc_same_core::retention::{self, Kept};
use cc_same_core::snapshot::{self, Manifest};
use cc_same_core::{ActionKind, Ctx, apply, desktop, fsx, service, watch};
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Global};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

#[cfg(windows)]
const AGENT_NAME: &str = "cc-same-agent.exe";
#[cfg(not(windows))]
const AGENT_NAME: &str = "cc-same-agent";

/// Something the user should hear about: the outcome of a task they started.
pub enum StoreEvent {
    Done(Result<String, String>),
}

/// A switch waiting for Claude to quit: how that goes, and the way to stop waiting.
#[derive(Debug, Default)]
pub struct Waiting {
    quitting: AtomicU8,
    stop: AtomicBool,
}

impl Waiting {
    pub fn new(quitting: Quitting) -> Waiting {
        let waiting = Waiting::default();
        waiting.set(quitting);
        waiting
    }

    fn set(&self, quitting: Quitting) {
        let n = match quitting {
            Quitting::Asked => 0,
            Quitting::Confirming => 1,
            Quitting::AfterWork => 2,
        };
        self.quitting.store(n, Ordering::Relaxed);
    }

    pub fn quitting(&self) -> Quitting {
        match self.quitting.load(Ordering::Relaxed) {
            1 => Quitting::Confirming,
            2 => Quitting::AfterWork,
            _ => Quitting::Asked,
        }
    }
}

/// Claude was restarted on its sign-in page, to sign in to another account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigningIn {
    /// The account set aside to make room, to switch back to.
    pub previous: Option<String>,
    /// The account the user meant to sign in to, by name, when they picked one.
    pub expected: Option<String>,
}

pub struct Store {
    pub ctx: Arc<Ctx>,
    pub overview: Option<Arc<Overview>>,
    pub snapshots: Vec<Manifest>,
    pub busy: Option<Busy>,
    pub cli_accounts: Option<Result<Vec<cc_same_core::cli_accounts::Account>, String>>,
    pub cli_switching: Option<u64>,
    cli_loading: bool,
    cli_usage_revision: u64,
    pub cli_usage: cc_same_core::cli_usage::Cache,
    pub usage_loading: bool,
    pub usage_error: Option<String>,
    /// The app opens (hidden) at login.
    pub open_at_login: bool,
    /// When the background switch was last turned on: the agent gets a moment to start.
    pub switched_on_at: Option<Instant>,
    /// The account a switch is heading for.
    pub switching_to: Option<String>,
    /// Waiting for a sign-in to another account in Claude.
    pub signing_in: Option<SigningIn>,
    /// A switch is waiting for Claude to quit.
    pub waiting: Option<Arc<Waiting>>,
    refreshing: bool,
    /// The background agent was compared with this version once.
    agent_checked: bool,
    fingerprint: u64,
    pending_since: Option<Instant>,
}

impl EventEmitter<StoreEvent> for Store {}

struct GlobalStore(Entity<Store>);

impl Global for GlobalStore {}

impl Store {
    fn blank(ctx: Arc<Ctx>) -> Store {
        Store {
            ctx,
            overview: None,
            snapshots: Vec::new(),
            busy: None,
            cli_accounts: None,
            cli_switching: None,
            cli_loading: false,
            cli_usage: Default::default(),
            cli_usage_revision: 0,
            usage_loading: false,
            usage_error: None,
            open_at_login: false,
            switched_on_at: None,
            switching_to: None,
            signing_in: None,
            waiting: None,
            refreshing: false,
            agent_checked: false,
            fingerprint: 0,
            pending_since: None,
        }
    }

    /// The live store: reads now, then again whenever anything it depends on changes.
    pub fn init(ctx: Arc<Ctx>, cx: &mut App) -> Entity<Store> {
        let store = cx.new(|cx| {
            let mut store = Store::blank(ctx);
            store.open_at_login = crate::login::enabled();
            // Finish an account switch a crash cut short, before Claude is next opened.
            let ctx = store.ctx.clone();
            cx.background_executor().spawn(async move { logins::recover_if_idle(&ctx) }).detach();
            store.refresh(cx);
            store.refresh_cli(cx);
            store.watch_for_changes(cx);
            store
        });
        cx.set_global(GlobalStore(store.clone()));
        store
    }

    /// A still picture of a given state (screenshots): no polling and no disk access.
    pub fn preview(
        ctx: Arc<Ctx>,
        overview: Option<Overview>,
        snapshots: Vec<Manifest>,
        busy: Option<Busy>,
        cx: &mut App,
    ) -> Entity<Store> {
        let store = cx.new(|_| Store { overview: overview.map(Arc::new), snapshots, busy, ..Store::blank(ctx) });
        cx.set_global(GlobalStore(store.clone()));
        store
    }

    pub fn global(cx: &App) -> Entity<Store> {
        cx.global::<GlobalStore>().0.clone()
    }

    pub fn config(&self) -> Config {
        self.ctx.config()
    }

    pub fn theme(&self) -> ThemeChoice {
        ThemeChoice::from_config(&self.ctx.config().appearance)
    }

    pub fn mood(&self) -> Mood {
        model::mood(self.overview.as_deref(), self.busy == Some(Busy::Syncing), self.pending_since.map(|t| t.elapsed()))
    }

    pub fn accounts(&self) -> Vec<Account> {
        self.overview.as_deref().map(model::accounts).unwrap_or_default()
    }

    /// The background agent is registered.
    pub fn background_on(&self) -> bool {
        self.overview.as_ref().is_some_and(|o| o.service.installed)
    }

    // ------------------------------------------------------------------ reading

    pub fn refresh_cli(&mut self, cx: &mut Context<Self>) {
        if self.cli_loading || self.cli_switching.is_some() {
            return;
        }
        self.cli_loading = true;
        let revision = self.cli_usage_revision;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async {
                    std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string()).and_then(|home| {
                        cc_same_core::cli_accounts::list(std::path::Path::new(&home)).map_err(|e| format!("{e:#}"))
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cli_loading = false;
                if revision != this.cli_usage_revision {
                    this.refresh_cli(cx);
                    return;
                }
                this.cli_accounts = Some(result);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn invalidate_cli_usage(&mut self, cx: &mut Context<Self>) {
        self.cli_usage_revision += 1;
        self.cli_usage = Default::default();
        self.refresh_cli(cx);
        self.refresh_usage(cx);
    }

    pub fn refresh_usage(&mut self, cx: &mut Context<Self>) {
        if self.usage_loading {
            return;
        }
        self.usage_loading = true;
        self.refresh(cx);
        let revision = self.cli_usage_revision;
        let mut cache = self.cli_usage.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (cache, accounts) = cx
                .background_executor()
                .spawn(async move {
                    let accounts =
                        std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string()).and_then(|home| {
                            cc_same_core::cli_accounts::list(std::path::Path::new(&home)).map_err(|e| format!("{e:#}"))
                        });
                    if let Ok(accounts) = &accounts {
                        cache.refresh(accounts);
                    }
                    (cache, accounts)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if revision != this.cli_usage_revision {
                    this.usage_loading = false;
                    this.refresh_usage(cx);
                    return;
                }
                this.cli_usage = cache;
                this.usage_error = accounts.as_ref().err().cloned();
                this.cli_accounts = Some(accounts);
                this.usage_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn switch_cli(&mut self, slot: u64, cx: &mut Context<Self>) {
        if self.cli_switching.is_some() || self.cli_loading {
            return;
        }
        self.cli_switching = Some(slot);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    std::env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string()).and_then(|home| {
                        cc_same_core::cli_accounts::switch(std::path::Path::new(&home), slot)
                            .map_err(|e| format!("{e:#}"))
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cli_switching = None;
                if result.is_ok() {
                    this.invalidate_cli_usage(cx);
                } else {
                    this.refresh_cli(cx);
                }
                cx.emit(StoreEvent::Done(result.map(|()| format!("CLI switched to account {slot}"))));
                cx.notify();
            });
        })
        .detach();
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let ctx = self.ctx.clone();
        cx.spawn(async move |this, cx| {
            let (overview, snapshots) =
                cx.background_executor().spawn(async move { (report::overview(&ctx), snapshot::list(&ctx)) }).await;
            let _ = this.update(cx, |this, cx| {
                this.refreshing = false;
                this.pending_since = match (overview.plan.total > 0, this.pending_since) {
                    (true, None) => Some(Instant::now()),
                    (true, since) => since,
                    (false, _) => None,
                };
                let signed_in = overview.logins.signed_in.clone();
                this.overview = Some(Arc::new(overview));
                this.snapshots = snapshots;
                this.renew_agent(cx);
                // Claude was waiting on its sign-in page, and now someone signed in.
                if this.busy.is_none()
                    && let (Some(_), Some(account)) = (&this.signing_in, signed_in)
                {
                    this.signing_in = None;
                    let name = this.account_name(&account);
                    cx.emit(StoreEvent::Done(Ok(tf("toast.added", &[("account", &name)]))));
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Re-read when Claude's session folders, Claude itself, our state or the agent change,
    /// and at least every 12 seconds.
    fn watch_for_changes(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut ticks = 0u32;
            loop {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                ticks += 1;
                let Ok((ctx, last)) = this.update(cx, |this, _| (this.ctx.clone(), this.fingerprint)) else { break };
                let fp = cx.background_executor().spawn(async move { fingerprint(&ctx) }).await;
                if fp != last || ticks.is_multiple_of(8) {
                    let alive = this.update(cx, |this, cx| {
                        this.fingerprint = fp;
                        this.refresh(cx);
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    // ------------------------------------------------------------------ doing

    /// Run blocking work off the UI thread, then report and re-read.
    fn run_task(
        &mut self,
        busy: Busy,
        cx: &mut Context<Self>,
        work: impl FnOnce(&Ctx) -> anyhow::Result<String> + Send + 'static,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(busy);
        cx.notify();
        let ctx = self.ctx.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { work(&ctx) }).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = None;
                cx.emit(StoreEvent::Done(result.map_err(|e| format!("{e:#}"))));
                this.refresh(cx);
            });
        })
        .detach();
    }

    pub fn sync_now(&mut self, cx: &mut Context<Self>) {
        self.run_task(Busy::Syncing, cx, |ctx| {
            let (_, out) = apply::run_sync(ctx, "app")?;
            if !out.errors.is_empty() {
                anyhow::bail!("{}", tn("toast.write_errors", out.errors.len(), &[]));
            }
            let copied = out.applied.get(&ActionKind::WriteRecord).copied().unwrap_or(0);
            Ok(match (copied, out.applied_total()) {
                (0, 0) => t("toast.in_sync"),
                (0, n) => tn("toast.applied", n, &[]),
                (n, _) => tn("toast.copied", n, &[]),
            })
        });
    }

    pub fn set_background(&mut self, on: bool, cx: &mut Context<Self>) {
        self.switched_on_at = on.then(Instant::now);
        self.run_task(Busy::Service, cx, move |ctx| {
            if on {
                install_agent(ctx)?;
            } else {
                service::uninstall(ctx)?;
            }
            // The switch shows the result.
            Ok(String::new())
        });
    }

    pub fn set_open_at_login(&mut self, on: bool, cx: &mut Context<Self>) {
        self.open_at_login = on;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { crate::login::set(on) }).await;
            let _ = this.update(cx, |this, cx| {
                this.open_at_login = crate::login::enabled();
                if let Err(e) = result {
                    cx.emit(StoreEvent::Done(Err(format!("{e:#}"))));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn restore(&mut self, id: String, cx: &mut Context<Self>) {
        self.run_task(Busy::Restoring, cx, move |ctx| {
            snapshot::restore(ctx, &id)?;
            Ok(t("toast.restored"))
        });
    }

    pub fn fix_links(&mut self, cx: &mut Context<Self>) {
        self.run_task(Busy::Fixing, cx, |ctx| {
            let fixed = snapshot::fix_symlinks(ctx)?;
            apply::run_sync(ctx, "app")?;
            Ok(tn("toast.fixed", fixed.len(), &[]))
        });
    }

    /// Keep the transcripts behind Desktop sessions, whichever setting it takes.
    pub fn keep_transcripts(&mut self, cx: &mut Context<Self>) {
        self.run_task(Busy::Saving, cx, |ctx| {
            Ok(match retention::keep(&ctx.paths)? {
                Kept::TenYears => t("toast.kept"),
                Kept::AnyAge | Kept::Already => t("toast.kept_unlimited"),
            })
        });
    }

    /// Put back what CC Same changed in Claude Code's settings.
    pub fn undo_retention(&mut self, cx: &mut Context<Self>) {
        self.run_task(Busy::Saving, cx, |ctx| {
            retention::undo(&ctx.paths)?;
            Ok(t("toast.undone"))
        });
    }

    /// Set the background agent up again when it no longer matches this app (once, at the first
    /// reading): after an update it still runs the old version; it is installed but not loaded
    /// (a restart that failed); or, on macOS, 0.1.0 and 0.1.1 registered a copy of the app that the
    /// system refuses to run. Quietly: the background switch shows how that went.
    fn renew_agent(&mut self, cx: &mut Context<Self>) {
        let Some(overview) = self.overview.clone() else { return };
        if std::mem::replace(&mut self.agent_checked, true) || !overview.service.installed || cfg!(debug_assertions) {
            return;
        }
        let behind = overview
            .heartbeat
            .as_ref()
            .and_then(|h| semver::Version::parse(&h.version).ok())
            .is_some_and(|v| v < semver::Version::parse(cc_same_core::VERSION).expect("package version"));
        let unloaded = overview.service.loaded == Some(false);
        let outdated = agent_outdated(&overview.service.program, &self.ctx.paths.bin_dir());
        if !(behind || unloaded || outdated) || self.busy.is_some() {
            return;
        }
        self.busy = Some(Busy::Service);
        self.switched_on_at = Some(Instant::now());
        cx.notify();
        let ctx = self.ctx.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { install_agent(&ctx) }).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = None;
                if let Err(e) = result {
                    eprintln!("setting the background agent up again: {e:#}");
                }
                this.refresh(cx);
            });
        })
        .detach();
    }

    // ------------------------------------------------------------------ accounts

    /// Switch Claude to `account`: Claude quits and starts again if it is open.
    pub fn switch_account(&mut self, account: String, cx: &mut Context<Self>) {
        self.run_switch(Some(account), None, cx);
    }

    /// Restart Claude on its sign-in page, to sign in to another account: the account in use is
    /// set aside, ready to switch back to. `expected` names the account the user means to add.
    pub fn sign_in_another(&mut self, expected: Option<String>, cx: &mut Context<Self>) {
        self.run_switch(None, expected, cx);
    }

    fn run_switch(&mut self, to: Option<String>, expected: Option<String>, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(Busy::Switching);
        self.switching_to = to.clone();
        let waiting = Arc::new(Waiting::default());
        self.waiting = Some(waiting.clone());
        cx.notify();
        let ctx = self.ctx.clone();
        cx.spawn(async move |this, cx| {
            let target = to.clone();
            let result = cx
                .background_executor()
                .spawn({
                    let waiting = waiting.clone();
                    async move {
                        let tell = |quitting: Quitting| waiting.set(quitting);
                        let watch = Watch { progress: Some(&tell), stop: Some(&waiting.stop) };
                        match &target {
                            // Switching from here is to use Claude now: it opens even if it was
                            // closed, where the switch alone leaves it closed.
                            Some(account) => accounts::switch(&ctx, account, &watch).and_then(|switched| {
                                if !switched.launched {
                                    desktop::launch(&ctx)?;
                                }
                                Ok(switched)
                            }),
                            None => accounts::add(&ctx, &watch),
                        }
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = None;
                this.switching_to = None;
                this.waiting = None;
                match result {
                    Ok(switched) => match &switched.to {
                        Some(account) => {
                            this.signing_in = None;
                            let name = this.account_name(account);
                            cx.emit(StoreEvent::Done(Ok(tf("toast.switched", &[("account", &name)]))));
                        }
                        None => this.signing_in = Some(SigningIn { previous: switched.from, expected }),
                    },
                    // Stopped by the user: nothing to apologize for.
                    Err(error) if error.downcast_ref::<NotReady>() == Some(&NotReady::Stopped) => {
                        let after_work = waiting.quitting() == Quitting::AfterWork;
                        cx.emit(StoreEvent::Done(Ok(t(if after_work {
                            "switch.stopped_after_work"
                        } else {
                            "switch.stopped"
                        }))));
                    }
                    Err(error) => cx.emit(StoreEvent::Done(Err(explain_switch(&error)))),
                }
                this.refresh(cx);
            });
        })
        .detach();
        // Show how Claude's quit goes as it changes.
        cx.spawn(async move |this, cx| {
            let mut shown = None;
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let going = this.update(cx, |this, cx| {
                    let quitting = this.waiting.as_ref().map(|w| w.quitting());
                    if quitting != shown {
                        shown = quitting;
                        cx.notify();
                    }
                    quitting.is_some()
                });
                if !matches!(going, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    /// How Claude's quit goes while a switch waits for it.
    pub fn quitting(&self) -> Option<Quitting> {
        self.waiting.as_ref().map(|w| w.quitting())
    }

    /// Stop waiting for Claude to quit: the switch ends there, with nothing changed.
    pub fn stop_waiting(&mut self, cx: &mut Context<Self>) {
        if let Some(waiting) = &self.waiting {
            waiting.stop.store(true, Ordering::Relaxed);
            cx.notify();
        }
    }

    /// The account "Next Account" switches to: the next one in the list that Claude can switch to
    /// and that takes part.
    pub fn next_account(&self) -> Option<String> {
        let ov = self.overview.as_deref()?;
        let can_switch = |s: &accounts::Slot| ov.logins.saved(&s.account).is_some();
        let current = ov.logins.signed_in.as_deref();
        let next =
            accounts::pick(&ov.roster, current, accounts::Strategy::Next, can_switch, &ov.usage, fsx::now_secs());
        next.map(|s| s.account.clone())
    }

    /// Take an account off the list, forgetting the sign-in kept for it.
    pub fn remove_account(&mut self, account: String, cx: &mut Context<Self>) {
        let name = self.account_name(&account);
        self.run_task(Busy::Saving, cx, move |ctx| {
            accounts::remove(ctx, &account)?;
            Ok(tf("toast.removed", &[("account", &name)]))
        });
    }

    /// Put an account in "Next Account", or leave it out.
    pub fn set_in_rotation(&mut self, account: String, on: bool, cx: &mut Context<Self>) {
        self.run_task(Busy::Saving, cx, move |ctx| {
            accounts::set_disabled(ctx, &account, !on)?;
            Ok(String::new())
        });
    }

    /// Give an account a name (its alias in the list), or take it away. It only edits the list,
    /// so unlike the tasks above it does not wait for a switch or a sync to finish.
    pub fn set_alias(&mut self, account: String, alias: Option<String>, cx: &mut Context<Self>) {
        let ctx = self.ctx.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { accounts::set_alias(&ctx, &account, alias.as_deref()) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Err(e) = result {
                    cx.emit(StoreEvent::Done(Err(format!("{e:#}"))));
                }
                this.refresh(cx);
            });
        })
        .detach();
    }

    /// An account's name as the window shows it.
    pub fn account_name(&self, id: &str) -> String {
        if let Some(account) = self.accounts().into_iter().find(|a| a.id == id) {
            return account.name;
        }
        let saved = self.overview.as_ref().and_then(|o| o.logins.saved(id).and_then(|s| s.email.clone()));
        saved.unwrap_or_else(|| tf("account.unnamed", &[("id", &cc_same_core::short(id))]))
    }

    /// Change a setting now and save it in the background.
    pub fn update_config(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Config)) {
        let mut cfg = self.ctx.config();
        change(&mut cfg);
        self.ctx.set_config(cfg.clone());
        cx.notify();
        let ctx = self.ctx.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(e) = cfg.save(&ctx.paths) {
                    ctx.log(format!("saving settings: {e:#}"));
                }
            })
            .detach();
    }
}

/// Register this app as the background agent and start it.
fn install_agent(ctx: &Ctx) -> anyhow::Result<()> {
    if !ctx.paths.config_file().exists() {
        ctx.config().save(&ctx.paths)?;
    }
    let exe = std::env::current_exe()?;
    if cfg!(target_os = "macos") {
        // The app's signature covers its executable only inside the app: macOS refuses to run a
        // copy, so the agent runs this very file, which has to stay.
        let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
        if crate::login::temporary_place(&exe) {
            anyhow::bail!("{}", t("background.move_first"));
        }
        service::install_in_place(ctx, &exe)?;
        // The copy an earlier version ran is of no more use.
        let _ = std::fs::remove_file(ctx.paths.bin_dir().join(AGENT_NAME));
    } else {
        service::install(ctx, &exe, AGENT_NAME)?;
    }
    Ok(())
}

/// Why a switch did not happen, in words people can act on.
fn explain_switch(error: &anyhow::Error) -> String {
    if let Some(ListRefused::NoOther) = error.downcast_ref::<ListRefused>() {
        return t("switch.no_other");
    }
    if let Some(refused) = error.downcast_ref::<Refused>() {
        return t(match refused {
            Refused::Unsupported => "switch.unsupported",
            Refused::NothingSaved => "switch.nothing_saved",
        });
    }
    if let Some(not_ready) = error.downcast_ref::<NotReady>() {
        return t(match not_ready {
            NotReady::StillOpen => "switch.still_open",
            NotReady::Updating => "switch.updating",
            NotReady::Stopped => "switch.stopped",
        });
    }
    format!("{error:#}")
}

/// Whether the registered agent has to be set up again from this app, on macOS: it is the copy
/// in `bin` an earlier version made, or the app it ran from is gone (moved or deleted).
fn agent_outdated(program: &[String], bin: &Path) -> bool {
    let Some(registered) = program.first().map(Path::new) else { return false };
    cfg!(target_os = "macos")
        && (registered == bin.join(AGENT_NAME)
            || (registered.to_string_lossy().contains(".app/Contents/MacOS/") && !registered.exists()))
}

/// Changes worth re-reading for: Claude's session folders and state, and ours (the account list
/// included).
fn fingerprint(ctx: &Ctx) -> u64 {
    let mut h = DefaultHasher::new();
    watch::fingerprint(ctx, desktop::is_running(ctx)).hash(&mut h);
    let paths = &ctx.paths;
    let files = [
        paths.state_file(),
        paths.heartbeat_file(),
        paths.config_file(),
        paths.claude_settings.clone(),
        paths.roster_file(),
    ];
    for p in files {
        if let Ok(md) = std::fs::metadata(&p) {
            fsx::mtime_ns(&md).hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_copied_by_an_earlier_version_or_left_behind_is_set_up_again() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        let program = |path: &Path| vec![path.to_string_lossy().into_owned(), "watch".to_string()];
        let expected = cfg!(target_os = "macos");
        assert_eq!(agent_outdated(&program(&bin.join(AGENT_NAME)), &bin), expected);
        assert_eq!(agent_outdated(&program(&dir.path().join("Gone.app/Contents/MacOS/CC Same")), &bin), expected);
        // The app it runs from is still there, or it is the command-line tool's own copy.
        let app = dir.path().join("CC Same.app/Contents/MacOS/CC Same");
        std::fs::create_dir_all(app.parent().unwrap()).unwrap();
        std::fs::write(&app, "").unwrap();
        assert!(!agent_outdated(&program(&app), &bin));
        assert!(!agent_outdated(&program(&bin.join("cc-same")), &bin));
        assert!(!agent_outdated(&[], &bin));
    }
}
