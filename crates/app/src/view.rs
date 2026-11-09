//! Desktop and CLI tabs, inline usage, and the current-account and session-sync cards.

use crate::i18n::{t, tf, tn};
use crate::model::{self, Account, Busy, Login};
use crate::settings;
use crate::store::{Store, StoreEvent};
use crate::theme;
use cc_same_core::desktop::Quitting;
use cc_same_core::report::Warning;
use cc_same_core::retention::Limit;
use cc_same_core::snapshot::Manifest;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, Theme, TitleBar, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    Anchor, AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, actions, div, prelude::FluentBuilder as _, px, rems,
};
use std::rc::Rc;

actions!(cc_same, [Refresh, SyncNow, OpenSettings, ShowTutorial]);

pub struct UniApp {
    store: Entity<Store>,
    page: usize,
    tutorial: crate::tutorial::Tutorial,
    tutorial_bounds: Rc<std::cell::Cell<Option<gpui_kit::Bounds<gpui_kit::Pixels>>>>,
    search: Option<Entity<InputState>>,
    sort_usage: bool,
    content_scroll: gpui_kit::ScrollHandle,
    mark: Option<std::sync::Arc<gpui_kit::RenderImage>>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl UniApp {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> UniApp {
        let mut subscriptions = vec![
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe_in(&store, window, |_, _, event: &StoreEvent, window, cx| {
                let StoreEvent::Done(result) = event;
                show_outcome(result, window, cx);
            }),
        ];
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t("ui.search")));
        subscriptions.push(cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()));
        cx.on_next_frame(window, |this, _, cx| {
            this.store.update(cx, |store, cx| store.refresh_usage(cx));
            cx.spawn(async move |this, cx| {
                let mark = cx
                    .background_executor()
                    .spawn(async {
                        crate::theme::load_mark(std::path::Path::new("/Applications/Claude.app/Contents/Resources"))
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.mark = mark;
                    cx.notify();
                });
            })
            .detach();
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        UniApp {
            tutorial_bounds: Default::default(),
            tutorial: crate::tutorial::Tutorial::new(store.read(cx).config().tutorial_done),
            store,
            page: 0,
            search: Some(search),
            sort_usage: false,
            content_scroll: gpui_kit::ScrollHandle::new(),
            mark: None,
            focus,
            _subscriptions: subscriptions,
        }
    }

    /// A still picture (screenshots): everything already in place.
    pub fn still(store: Entity<Store>, cx: &mut Context<Self>) -> UniApp {
        UniApp {
            tutorial_bounds: Default::default(),
            tutorial: crate::tutorial::Tutorial::default(),
            store,
            page: 0,
            search: None,
            sort_usage: false,
            content_scroll: gpui_kit::ScrollHandle::new(),
            mark: None,
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
        }
    }

    pub fn store(&self) -> &Entity<Store> {
        &self.store
    }

    pub fn show_tutorial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.close_sheet(cx);
        self.tutorial.replay();
        self.content_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        self.page = 0;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn dismiss_tutorial(&mut self, cx: &mut Context<Self>) {
        let mut done = self.store.read(cx).config().tutorial_done;
        self.tutorial.dismiss(&mut done);
        self.store.update(cx, |store, cx| store.update_config(cx, |cfg| cfg.tutorial_done = done));
        cx.notify();
    }

    fn advance_tutorial(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(step) = self.tutorial.step {
            if !back && step == 4 {
                self.dismiss_tutorial(cx);
                return;
            }
            let step = if back { step.saturating_sub(1) } else { step + 1 };
            self.tutorial.step = Some(step);
            self.content_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
            self.page = usize::from(step == 3);
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn tutorial_target_present(&self, cx: &App) -> bool {
        let store = self.store.read(cx);
        match self.tutorial.step {
            Some(1) => store.accounts().iter().any(|a| a.login == Login::SignedIn || a.open),
            Some(2) => !store.accounts().is_empty(),
            _ => true,
        }
    }

    fn coach(&self, step: usize, element: impl IntoElement, cx: &App) -> Div {
        let bounds = self.tutorial_bounds.clone();
        div().relative().child(element).when(
            self.tutorial.step == Some(step) && self.tutorial_target_present(cx),
            |el| {
                el.child(
                    gpui_kit::canvas(
                        move |rect, window, _| bounds.set(Some(rect.intersect(&window.content_mask().bounds))),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
            },
        )
    }

    fn render_tutorial(&self, cx: &Context<Self>) -> AnyElement {
        let step = self.tutorial.step.unwrap_or(0);
        let present = self.tutorial_target_present(cx);
        let target = self.tutorial_bounds.clone();
        let accent = theme::accent_text(cx);
        div()
            .id("tutorial-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                }),
            )
            .child(
                gpui_kit::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        use gpui_kit::{Bounds, fill, point, size};
                        let shade = gpui_kit::rgba(0x00000099);
                        if let Some(hole) = target.get().filter(|_| present) {
                            let hole = hole.intersect(&bounds);
                            let rects = [
                                Bounds::new(bounds.origin, size(bounds.size.width, hole.top() - bounds.top())),
                                Bounds::new(
                                    point(bounds.left(), hole.bottom()),
                                    size(bounds.size.width, bounds.bottom() - hole.bottom()),
                                ),
                                Bounds::new(
                                    point(bounds.left(), hole.top()),
                                    size(hole.left() - bounds.left(), hole.size.height),
                                ),
                                Bounds::new(
                                    point(hole.right(), hole.top()),
                                    size(bounds.right() - hole.right(), hole.size.height),
                                ),
                            ];
                            for rect in rects {
                                window.paint_quad(fill(rect, shade));
                            }
                            window.paint_quad(gpui_kit::outline(hole, accent, gpui_kit::BorderStyle::Solid));
                        } else {
                            window.paint_quad(fill(bounds, shade));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(
                v_flex()
                    .w(px(580.))
                    .when(present, |el| el.absolute().left(px(50.)).right(px(50.)))
                    .when(present && step == 2, |el| el.top(px(100.)))
                    .when(present && step != 2, |el| el.bottom(px(28.)))
                    .p_5()
                    .gap_4()
                    .rounded(px(16.))
                    .shadow_lg()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(theme::accent_text(cx))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().text_size(px(21.)).child(t(&format!("tutorial.title{step}"))))
                            .child(format!("{} / 5", step + 1)),
                    )
                    .child(div().text_size(px(16.)).child(t(&format!("tutorial.body{step}"))))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                Button::new("tutorial-skip")
                                    .ghost()
                                    .label(t("tutorial.skip"))
                                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_tutorial(cx))),
                            )
                            .child(h_flex().gap_2().children((0..5).map(|i| {
                                div().size(px(7.)).rounded_full().bg(if i == step {
                                    theme::accent_text(cx)
                                } else {
                                    cx.theme().muted_foreground
                                })
                            })))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("tutorial-back")
                                            .outline()
                                            .label(t("tutorial.back"))
                                            .disabled(step == 0)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.advance_tutorial(true, window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("tutorial-next")
                                            .primary()
                                            .bg(theme::jelly())
                                            .label(t(if step == 4 { "tutorial.finish" } else { "tutorial.next" }))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.advance_tutorial(false, window, cx)
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    // ------------------------------------------------------------------ actions

    fn sync_now(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.sync_now(cx));
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tutorial.step.is_none() {
            settings::open(self.store.clone(), window, cx);
        }
    }

    pub fn confirm_restore(&mut self, snapshot: &Manifest, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let id = snapshot.id.clone();
        let when = model::snapshot_when(&snapshot.id);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (store, id) = (store.clone(), id.clone());
            alert
                .title(tf("restore.title", &[("when", &when)]))
                .description(format!("{} {}", t("restore.body"), t("settings.quit_to_restore")))
                .confirm()
                .ok_text(t("restore.ok"))
                .cancel_text(t("dialog.cancel"))
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    let id = id.clone();
                    store.update(cx, |store, cx| store.restore(id, cx));
                    true
                })
        });
    }

    pub fn confirm_fix_links(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let store = store.clone();
            alert
                .title(t("fix.title"))
                .description(t("fix.body"))
                .confirm()
                .ok_text(t("fix.ok"))
                .cancel_text(t("dialog.cancel"))
                .on_ok(move |_, _, cx| {
                    store.update(cx, |store, cx| store.fix_links(cx));
                    true
                })
        });
    }

    /// What sits at the end of an account's row: where Claude stands with it, or what one click
    /// does.
    fn account_action(&self, a: &Account, cx: &Context<Self>) -> Option<AnyElement> {
        let (busy, switching_to, signing_in) = {
            let store = self.store.read(cx);
            (store.busy, store.switching_to.clone(), store.signing_in.is_some())
        };
        let theme = cx.theme();
        let tag = |label: String, dot_color: Option<Hsla>| {
            Tag::secondary()
                .small()
                .rounded_full()
                .child(h_flex().gap_1p5().children(dot_color.map(dot)).child(label))
                .into_any_element()
        };
        if a.open {
            return Some(tag(t("account.open"), Some(theme.primary)));
        }
        if a.login == Login::SignedIn {
            return Some(tag(t("account.signed_in"), None));
        }
        if switching_to.as_deref() == Some(a.id.as_str()) {
            let muted = theme.muted_foreground;
            return Some(
                h_flex()
                    .gap_1p5()
                    .text_size(rems(0.923))
                    .text_color(muted)
                    .child(Spinner::new().xsmall())
                    .child(t("account.switching"))
                    .into_any_element(),
            );
        }
        let id = a.id.clone();
        match a.login {
            Login::Saved { stale } => Some(
                Button::new(SharedString::from(format!("switch-{}", a.id)))
                    .small()
                    .outline()
                    .label(t("account.switch"))
                    .disabled(busy.is_some())
                    .when(stale, |b| b.tooltip(t("account.stale")))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let id = id.clone();
                        this.store.update(cx, |store, cx| store.switch_account(id, cx));
                    }))
                    .into_any_element(),
            ),
            // Waiting for a sign-in already: one restart is enough.
            Login::Unknown if signing_in => None,
            Login::Unknown => {
                // Named in the dialog only by an email address, something people can type.
                let name = a.email.clone();
                Some(
                    Button::new(SharedString::from(format!("sign-in-{}", a.id)))
                        .small()
                        .ghost()
                        .text_color(theme::accent_text(cx))
                        .label(t("ui.relogin"))
                        .disabled(busy.is_some())
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.confirm_sign_in(name.clone(), window, cx)),
                        )
                        .into_any_element(),
                )
            }
            Login::SignedIn | Login::Unsupported => None,
        }
    }

    /// Explain, then restart Claude on its sign-in page. `expected` names the account the user
    /// picked, when they did.
    pub fn confirm_sign_in(&mut self, expected: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let current = {
            let store = store.read(cx);
            let signed_in = store.overview.as_ref().and_then(|o| o.logins.signed_in.clone());
            signed_in.map(|id| store.account_name(&id))
        };
        window.open_alert_dialog(cx, move |alert, _, cx| {
            let muted = cx.theme().muted_foreground;
            let (title, body) = match &expected {
                Some(name) => (tf("signin.title_as", &[("account", name)]), tf("signin.body_as", &[("account", name)])),
                None => (t("signin.title"), t("signin.body")),
            };
            let description = v_flex()
                .gap_2()
                .child(body)
                .when_some(current.as_ref(), |el, name| el.child(tf("signin.keeps", &[("account", name)])))
                .child(div().pt_1().text_size(rems(0.846)).text_color(muted).child(t("signin.tip")));
            let (store, expected) = (store.clone(), expected.clone());
            alert
                .title(title)
                .description(description)
                .confirm()
                .ok_text(t("signin.ok"))
                .cancel_text(t("dialog.cancel"))
                .on_ok(move |_, _, cx| {
                    let expected = expected.clone();
                    store.update(cx, |store, cx| store.sign_in_another(expected, cx));
                    true
                })
        });
    }

    /// Ask before switching Claude to `account`, as the menu bar does: a menu item is easy to hit
    /// by mistake, and switching restarts Claude. An account whose sign-in is kept switches; one
    /// without restarts Claude on its sign-in page instead, as its "Sign in" button does.
    pub fn confirm_switch(&mut self, account: String, window: &mut Window, cx: &mut Context<Self>) {
        let (saved, name, email, current, running) = {
            let store = self.store.read(cx);
            let Some(overview) = store.overview.as_ref() else { return };
            if store.busy.is_some() || overview.logins.signed_in.as_deref() == Some(account.as_str()) {
                return;
            }
            let saved = overview.logins.saved(&account).cloned();
            let email = store.accounts().into_iter().find(|a| a.id == account).and_then(|a| a.email);
            let email = email.or_else(|| saved.as_ref().and_then(|s| s.email.clone()));
            let current = overview.logins.signed_in.as_ref().map(|id| store.account_name(id));
            (saved, store.account_name(&account), email, current, overview.app.running)
        };
        let Some(saved) = saved else {
            return self.confirm_sign_in(email, window, cx);
        };
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, cx| {
            let muted = cx.theme().muted_foreground;
            let body = if running { "switch.confirm_body" } else { "switch.confirm_body_closed" };
            let description = v_flex()
                .gap_2()
                .child(tf(body, &[("account", &name)]))
                .when_some(current.as_ref(), |el, current| el.child(tf("signin.keeps", &[("account", current)])))
                .when(saved.stale(), |el| {
                    el.child(div().pt_1().text_size(rems(0.846)).text_color(muted).child(t("account.stale")))
                });
            let (store, account) = (store.clone(), account.clone());
            alert
                .title(tf("switch.confirm_title", &[("account", &name)]))
                .description(description)
                .confirm()
                .ok_text(t("account.switch"))
                .cancel_text(t("dialog.cancel"))
                .on_ok(move |_, _, cx| {
                    let account = account.clone();
                    store.update(cx, |store, cx| store.switch_account(account, cx));
                    true
                })
        });
    }

    /// Ask, then take an account off the list (forgetting the sign-in kept for it).
    pub fn confirm_remove(&mut self, account: String, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (store, account) = (store.clone(), account.clone());
            alert
                .title(tf("remove.title", &[("account", &name)]))
                .description(t("remove.body"))
                .confirm()
                .ok_text(t("remove.ok"))
                .cancel_text(t("dialog.cancel"))
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    let account = account.clone();
                    store.update(cx, |store, cx| store.remove_account(account, cx));
                    true
                })
        });
    }

    /// Ask for the name an account goes by in the list (its alias). Empty takes the name away.
    pub fn open_name(&mut self, account: String, window: &mut Window, cx: &mut Context<Self>) {
        let (title, current, others) = {
            let rows = self.store.read(cx).accounts();
            let Some(row) = rows.iter().find(|a| a.id == account) else { return };
            // An account as people know it apart from its name: its email, else its ID.
            let known_as = |a: &Account| {
                a.email.clone().unwrap_or_else(|| tf("account.unnamed", &[("id", &cc_same_core::short(&a.id))]))
            };
            let others: Vec<(String, String)> =
                rows.iter().filter(|a| a.id != account).filter_map(|a| Some((a.alias.clone()?, known_as(a)))).collect();
            (tf("name.title", &[("account", &known_as(row))]), row.alias.clone(), Rc::new(others))
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t("name.placeholder")).default_value(current.unwrap_or_default())
        });
        let problem = cx.new(|_| None::<String>);
        // Editing clears the complaint about what was there before.
        cx.subscribe(&input, {
            let problem = problem.clone();
            move |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    problem.update(cx, |problem, cx| {
                        if problem.take().is_some() {
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
        let (store, field) = (self.store.clone(), input.clone());
        window.open_alert_dialog(cx, move |alert, _, cx| {
            let theme = cx.theme();
            let note = match problem.read(cx) {
                Some(problem) => div().text_color(theme.danger).child(problem.clone()),
                None => div()
                    .text_color(theme.muted_foreground)
                    .child(tf("name.hint", &[("max", &cc_same_core::accounts::ALIAS_MAX)])),
            };
            // Below the description, not in it: the field keeps its own color and line height. The
            // dialog clips its body, so the field's focus ring needs room above it.
            let field_and_note = v_flex()
                .pt_1()
                .gap_2()
                // Its default 8 px padding leaves a 20 px line 16 px and cuts off descenders.
                .child(Input::new(&field).cleanable(true).py_1())
                .child(note.text_size(rems(0.846)).line_height(rems(1.2)));
            let (input, problem, store, account, others) =
                (field.clone(), problem.clone(), store.clone(), account.clone(), others.clone());
            alert
                .title(title.clone())
                .description(t("name.body"))
                .child(field_and_note)
                .confirm()
                .ok_text(t("name.ok"))
                .cancel_text(t("dialog.cancel"))
                .on_ok(move |_, window, cx| {
                    let text = input.read(cx).value();
                    if let Some(found) = model::name_problem(&text, &others) {
                        problem.update(cx, |problem, cx| {
                            *problem = Some(found);
                            cx.notify();
                        });
                        window.refresh();
                        return false;
                    }
                    let name = Some(text.trim().to_string()).filter(|name| !name.is_empty());
                    let account = account.clone();
                    store.update(cx, |store, cx| store.set_alias(account, name, cx));
                    true
                })
        });
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    /// Ask, then switch Claude to the next account in the list.
    pub fn confirm_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).next_account() {
            Some(next) => self.confirm_switch(next, window, cx),
            None => show_outcome(&Err(t("switch.no_other")), window, cx),
        }
    }

    fn render_pages(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let count = if self.page == 0 {
            store.accounts().len()
        } else {
            store.cli_accounts.as_ref().and_then(|a| a.as_ref().ok()).map_or(0, Vec::len)
        };
        h_flex()
            .px(px(18.))
            .pt(px(14.))
            .gap_2()
            .justify_between()
            .child(self.coach(
                0,
                h_flex().p(px(3.)).gap(px(3.)).rounded(px(10.)).bg(cx.theme().muted).children((0..2).map(|index| {
                    Button::new(if index == 0 { "desktop-tab" } else { "cli-tab" })
                        .ghost()
                        .w(px(if count > 4 { 83. } else { 113. }))
                        .h(px(32.))
                        .label(t(if index == 0 { "page.desktop" } else { "page.cli" }))
                        .bg(if self.page == index {
                            if cx.theme().is_dark() { theme::empty(cx) } else { cx.theme().background }
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .text_color(if self.page == index {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.page = index;
                            cx.notify();
                        }))
                })),
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .when(count > 4, |el| {
                        el.children(
                            self.search
                                .as_ref()
                                .map(|input| div().w(px(120.)).child(Input::new(input).small().cleanable(true))),
                        )
                    })
                    .child(Button::new("sort-usage").small().outline().label(t("ui.sort")).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.sort_usage = true;
                            cx.notify();
                        },
                    )))
                    .child(self.coach(
                        3,
                        Button::new("add-account").small().primary().label(t("ui.add")).bg(theme::jelly()).on_click(
                            cx.listener(|this, _, window, cx| {
                                if this.page == 1 {
                                    crate::cli_login::open(this.store.clone(), None, window, cx);
                                } else {
                                    this.confirm_sign_in(None, window, cx);
                                }
                            }),
                        ),
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let latest = store
            .cli_usage
            .readings
            .values()
            .filter_map(|r| r.usage.map(|u| u.at))
            .chain(store.overview.iter().flat_map(|o| o.usage.values().map(|u| u.at)))
            .max_by(f64::total_cmp);
        let updated =
            latest.map(|at| tf("ui.updated", &[("ago", &crate::i18n::ago(at))])).unwrap_or_else(|| t("ui.noReading"));
        TitleBar::new()
            .h(px(52.))
            .bg(cx.theme().background)
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .pr_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_size(px(17.))
                            .child("CCSwift")
                            .children(self.mark.clone().map(|mark| gpui_kit::img(mark).size(px(17.)))),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().text_size(px(13.)).text_color(cx.theme().muted_foreground).child(updated))
                            .child(
                                Button::new("refresh")
                                    .ghost()
                                    .small()
                                    .icon(IconName::RefreshCw)
                                    .tooltip(t("ui.refresh"))
                                    .disabled(store.usage_loading)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.store.update(cx, |s, cx| s.refresh_usage(cx))
                                        }),
                                    ),
                            )
                            .child(
                                Button::new("open-settings")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Settings2)
                                    .tooltip(t("settings.title"))
                                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_current(&self, accounts: &[Account], cx: &Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let cli = store.cli_accounts.as_ref().and_then(|r| r.as_ref().ok());
        let current_cli = cli.and_then(|rows| rows.iter().find(|a| a.active));
        let current_desktop = accounts.iter().find(|a| a.login == Login::SignedIn || a.open);
        let email =
            if self.page == 1 { current_cli.map(|a| a.email.clone()) } else { current_desktop.map(account_email) };
        let email = email.unwrap_or_else(|| t("ui.noCurrent"));
        let subtitle = if self.page == 1 {
            current_cli
                .map(|a| {
                    format!(
                        "{} · {} · {}",
                        tf("ui.slot", &[("n", &a.slot)]),
                        cli_status(store.cli_usage.readings.get(&a.slot)),
                        t("ui.desktopUnaffected")
                    )
                })
                .unwrap_or_else(|| t("ui.desktopUnaffected"))
        } else if store.mood().settled() {
            tf("ui.heroSubDesk", &[("plan", &"—"), ("n", &accounts.len())])
        } else {
            format!("— · {}", store.mood().title())
        };
        let next_cli = cli.and_then(|rows| {
            let at = rows.iter().position(|a| a.active).unwrap_or(rows.len().saturating_sub(1));
            (1..=rows.len()).map(|offset| &rows[(at + offset) % rows.len()]).find(|a| !a.active).map(|a| a.slot)
        });
        let can_next = if self.page == 1 {
            next_cli.is_some() && store.cli_switching.is_none()
        } else {
            store.next_account().is_some() && store.busy.is_none()
        };
        h_flex()
            .gap(px(14.))
            .p(px(16.))
            .rounded(px(16.))
            .bg(theme::jelly())
            .text_color(gpui_kit::white())
            .border_1()
            .border_color(gpui_kit::white().opacity(0.35))
            .shadow(theme::jelly_shadow())
            .child(
                avatar(&email, true)
                    .size(px(44.))
                    .text_size(px(21.))
                    .bg(gpui_kit::white().opacity(0.26))
                    .border_1()
                    .border_color(gpui_kit::white().opacity(0.5)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_size(px(12.5))
                            .child(dot(gpui_kit::white()))
                            .child(t(if self.page == 1 { "ui.heroCli" } else { "ui.heroDesk" })),
                    )
                    .child(div().text_size(px(23.)).truncate().child(email))
                    .child(div().text_size(px(13.)).truncate().child(subtitle)),
            )
            .child(
                Button::new("next-account")
                    .h(px(30.))
                    .small()
                    .outline()
                    .label(t("ui.next"))
                    .bg(gpui_kit::white().opacity(0.18))
                    .text_color(gpui_kit::white())
                    .border_color(gpui_kit::white().opacity(0.55))
                    .disabled(!can_next)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.page == 1 {
                            if let Some(slot) = next_cli {
                                this.store.update(cx, |s, cx| s.switch_cli(slot, cx));
                            }
                        } else {
                            this.confirm_next(window, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    fn render_background(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let on = store.background_on();
        let disabled = store.busy.is_some() || store.overview.is_none();
        let detail = store
            .overview
            .as_ref()
            .and_then(|o| o.last_sync.as_ref())
            .map(|s| tf("ui.lastSync", &[("ago", &crate::i18n::ago(s.at))]))
            .unwrap_or_else(|| t("ui.notStarted"));
        let latest = store.snapshots.last().map(|s| model::snapshot_when(&s.id)).unwrap_or_else(|| t("ui.notStarted"));
        let backup = tf("ui.backupLine", &[("when", &latest), ("n", &store.snapshots.len())]);
        let line =
            |icon, title, detail, action: AnyElement| {
                h_flex()
                    .h(px(50.))
                    .px(px(14.))
                    .gap_3()
                    .child(Icon::new(icon).small().text_color(cx.theme().muted_foreground))
                    .child(v_flex().flex_1().min_w_0().child(title).child(
                        div().text_size(px(12.5)).text_color(cx.theme().muted_foreground).truncate().child(detail),
                    ))
                    .child(action)
            };
        v_flex()
            .rounded(px(14.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .child(line(
                IconName::RefreshCw,
                t("ui.sync"),
                format!("{detail} · {}", store.mood().title()),
                Button::new("sync-now")
                    .small()
                    .outline()
                    .label(t("ui.syncNow"))
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| this.sync_now(cx)))
                    .into_any_element(),
            ))
            .child(div().h(px(1.)).bg(cx.theme().border))
            .child(line(
                IconName::CircleCheck,
                t("ui.auto"),
                t(if on { "ui.autoOnText" } else { "ui.autoOff" }),
                Switch::new("background-sync")
                    .checked(on)
                    .disabled(disabled)
                    .on_click(
                        cx.listener(|this, on: &bool, _, cx| this.store.update(cx, |s, cx| s.set_background(*on, cx))),
                    )
                    .into_any_element(),
            ))
            .child(div().h(px(1.)).bg(cx.theme().border))
            .child(line(
                IconName::FolderOpen,
                t("ui.backup"),
                backup,
                Button::new("backups")
                    .small()
                    .outline()
                    .label(t("ui.manage"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        crate::backups::open(this.store.clone(), cx.entity(), window, cx)
                    }))
                    .into_any_element(),
            ))
            .into_any_element()
    }

    fn render_table(&self, accounts: &[Account], cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let query = self.search.as_ref().map(|s| s.read(cx).value().to_lowercase()).unwrap_or_default();
        let count = if self.page == 0 {
            accounts.len()
        } else {
            store.cli_accounts.as_ref().and_then(|r| r.as_ref().ok()).map_or(0, Vec::len)
        };
        let query = if count > 4 { query } else { String::new() };
        let mut rows = Vec::new();
        if self.page == 0 {
            let mut accounts = accounts.to_vec();
            if self.sort_usage {
                accounts.sort_by(|a, b| {
                    desktop_usage(store, b)
                        .0
                        .and_then(|u| u.five_hour)
                        .unwrap_or(-1.)
                        .total_cmp(&desktop_usage(store, a).0.and_then(|u| u.five_hour).unwrap_or(-1.))
                });
            }
            for a in accounts.iter().filter(|a| account_email(a).to_lowercase().contains(&query)) {
                let active = a.open || a.login == Login::SignedIn;
                let (usage, reading) = desktop_usage(store, a);
                let action = if active {
                    in_use(cx)
                } else {
                    self.account_action(a, cx).unwrap_or_else(|| div().into_any_element())
                };
                rows.push(account_row(
                    &account_email(a),
                    "—".into(),
                    active,
                    usage,
                    reading,
                    t(if a.login == Login::Unknown { "ui.none" } else { "ui.saved" }),
                    a.login == Login::Unknown,
                    action,
                    cx,
                ));
            }
        } else if let Some(Ok(accounts)) = &store.cli_accounts {
            let mut accounts = accounts.clone();
            if self.sort_usage {
                accounts.sort_by(|a, b| {
                    store
                        .cli_usage
                        .readings
                        .get(&b.slot)
                        .and_then(|r| r.usage)
                        .and_then(|u| u.five_hour)
                        .unwrap_or(-1.)
                        .total_cmp(
                            &store
                                .cli_usage
                                .readings
                                .get(&a.slot)
                                .and_then(|r| r.usage)
                                .and_then(|u| u.five_hour)
                                .unwrap_or(-1.),
                        )
                });
            }
            for a in accounts.iter().filter(|a| a.email.to_lowercase().contains(&query)) {
                let slot = a.slot;
                let reading = store.cli_usage.readings.get(&slot);
                let expired = reading.is_some_and(|r| r.status() == "Expired");
                let action = if expired {
                    Button::new(SharedString::from(format!("cli-relogin-{slot}")))
                        .small()
                        .ghost()
                        .text_color(theme::accent_text(cx))
                        .label(t("ui.relogin"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            crate::cli_login::open(this.store.clone(), Some(slot), window, cx)
                        }))
                        .into_any_element()
                } else if a.active {
                    in_use(cx)
                } else {
                    Button::new(SharedString::from(format!("cli-switch-{slot}")))
                        .small()
                        .outline()
                        .label(t(if store.cli_switching == Some(slot) { "account.switching" } else { "ui.switch" }))
                        .disabled(store.cli_switching.is_some())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.store.update(cx, |s, cx| s.switch_cli(slot, cx))),
                        )
                        .into_any_element()
                };
                rows.push(account_row(
                    &a.email,
                    tf("ui.slot", &[("n", &slot)]),
                    a.active,
                    reading.and_then(|r| r.usage),
                    reading,
                    cli_status(reading),
                    expired,
                    action,
                    cx,
                ));
            }
        }
        let mut table = v_flex().child(
            h_flex()
                .h(px(30.))
                .gap_3()
                .text_size(px(12.5))
                .text_color(cx.theme().muted_foreground)
                .border_b_1()
                .border_color(cx.theme().border)
                .child(div().flex_1().min_w_0().child(format!("{} · {count}", t("ui.colAccount"))))
                .child(div().w(px(112.)).child(t("ui.col5")))
                .child(div().w(px(112.)).child(t("ui.col7")))
                .child(div().w(px(96.)).child(t("ui.colState")))
                .child(div().w(px(90.)).text_right().child(t("ui.colAction"))),
        );
        if rows.is_empty() {
            let message = if self.page == 1 {
                match &store.cli_accounts {
                    None => t("cli.loading"),
                    Some(Err(e)) => e.clone(),
                    _ => t("ui.empty"),
                }
            } else {
                t("ui.empty")
            };
            table = table.child(div().py_4().text_color(cx.theme().muted_foreground).child(message));
        }
        table.children(rows).into_any_element()
    }

    fn render_notices(&self, accounts: &[Account], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut notices = Vec::new();
        let (overview, busy, log) = {
            let store = self.store.read(cx);
            (store.overview.clone(), store.busy, store.ctx.paths.log_file.clone())
        };
        let Some(ov) = overview else { return notices };
        notices.extend(self.render_sign_in_notice(cx));
        if let Some(hint) = ov.restart_hint() {
            let open =
                accounts.iter().find(|a| a.open).map(|a| a.name.clone()).unwrap_or_else(|| t("notice.open_account"));
            notices.push(notice(
                "notice-restart",
                Tone::Info,
                tn("notice.restart", hint.sessions as usize, &[("account", &open)]),
                Vec::new(),
                None,
                cx,
            ));
        }
        if let Some(ls) = &ov.last_sync
            && ls.errors > 0
        {
            notices.push(notice(
                "notice-errors",
                Tone::Danger,
                tn("notice.errors", ls.errors as usize, &[]),
                vec![
                    Button::new("open-log")
                        .small()
                        .outline()
                        .label(t("notice.show_log"))
                        .on_click(move |_, _, cx| cx.open_with_system(&log))
                        .into_any_element(),
                ],
                None,
                cx,
            ));
        }
        // Only when Claude Code will actually delete the transcripts behind Desktop sessions.
        if let Some((days, limited_by)) = ov.warnings.iter().find_map(|w| match w {
            Warning::ShortRetention { days, limited_by, .. } => Some((*days, *limited_by)),
            _ => None,
        }) {
            let mut text = tn("notice.retention", days.round() as usize, &[]);
            match limited_by {
                Limit::Organization => text = format!("{text} {}", t("notice.retention_org")),
                Limit::OlderClaudeCode => text = format!("{text} {}", t("notice.retention_old")),
                Limit::DesktopSetting => {}
            }
            let action = (limited_by != Limit::Organization).then(|| {
                let store = self.store.clone();
                let label = match limited_by {
                    Limit::OlderClaudeCode => t("notice.keep_10_years"),
                    _ => t("notice.keep_them"),
                };
                Button::new("keep-transcripts")
                    .small()
                    .outline()
                    .label(label)
                    .disabled(busy.is_some())
                    .on_click(move |_, _, cx| store.update(cx, |store, cx| store.keep_transcripts(cx)))
                    .into_any_element()
            });
            notices.push(notice("notice-retention", Tone::Warning, text, action.into_iter().collect(), None, cx));
        }
        notices
    }

    /// Claude is asking before it quits for a switch, restarting on its sign-in page, or waiting
    /// there for the user to sign in.
    fn render_sign_in_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (busy, switching_to, signing_in, quitting) = {
            let store = self.store.read(cx);
            (store.busy, store.switching_to.clone(), store.signing_in.clone(), store.quitting())
        };
        // Claude has work in progress: it asks whether to stop it, or waits for it. Either way the
        // user decides in Claude, and can stop waiting here (say after choosing Cancel there).
        if let Some(quitting @ (Quitting::Confirming | Quitting::AfterWork)) = quitting {
            let (tone, text) = match quitting {
                Quitting::Confirming => (Tone::Warning, t("quit.confirming")),
                _ => (Tone::Info, t("quit.after_work")),
            };
            let stop = Button::new("stop-waiting")
                .small()
                .outline()
                .label(t("quit.stop"))
                .on_click(cx.listener(|this, _, _, cx| this.store.update(cx, |store, cx| store.stop_waiting(cx))))
                .into_any_element();
            return Some(notice("notice-quitting", tone, text, vec![stop], None, cx));
        }
        if busy == Some(Busy::Switching) && switching_to.is_none() {
            let spinner = Spinner::new().small().into_any_element();
            return Some(notice("notice-sign-in", Tone::Info, t("signin.restarting"), vec![spinner], None, cx));
        }
        let pending = signing_in?;
        let text = match &pending.expected {
            Some(name) => tf("signin.pending_as", &[("account", name)]),
            None => t("signin.pending"),
        };
        let back = pending.previous.map(|previous| {
            Button::new("sign-in-back")
                .small()
                .outline()
                .label(t("signin.back"))
                .disabled(busy.is_some())
                .on_click(cx.listener(move |this, _, _, cx| {
                    let previous = previous.clone();
                    this.store.update(cx, |store, cx| store.switch_account(previous, cx));
                }))
                .into_any_element()
        });
        Some(notice("notice-sign-in", Tone::Info, text, back.into_iter().collect(), None, cx))
    }
}

impl Render for UniApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (choice, accounts) = {
            let store = self.store.read(cx);
            (store.theme(), store.accounts())
        };
        theme::follow(choice, window, cx);
        if let Some(search) = &self.search {
            let placeholder = t("ui.search");
            if search.read(cx).presentation().placeholder().as_ref() != placeholder {
                search.update(cx, |input, cx| input.set_placeholder(placeholder, window, cx));
            }
        }
        self.tutorial_bounds.set(None);
        let content = v_flex()
            .px(px(18.))
            .py(px(14.))
            .gap(px(12.))
            .child(self.coach(1, self.render_current(&accounts, cx), cx))
            .when(self.page == 0, |el| {
                let card = self.render_background(cx);
                el.child(self.coach(4, card, cx))
            })
            .child({
                let table = self.render_table(&accounts, cx);
                self.coach(2, table, cx)
            })
            .child(
                h_flex()
                    .justify_between()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t(if self.page == 0 { "ui.footDesk" } else { "ui.footCli" }))
                    .child(div().text_color(theme::accent_text(cx)).child(t("ui.legend"))),
            )
            .when(self.page == 0, |el| el.children(self.render_notices(&accounts, cx)));
        v_flex()
            .relative()
            .key_context("UniApp")
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                if this.tutorial.step.is_some() {
                    match event.keystroke.key.as_str() {
                        "right" => this.advance_tutorial(false, window, cx),
                        "left" => this.advance_tutorial(true, window, cx),
                        "escape" => this.dismiss_tutorial(cx),
                        _ => {}
                    }
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.store.update(cx, |s, cx| s.refresh_usage(cx))))
            .on_action(cx.listener(|this, _: &SyncNow, _, cx| this.sync_now(cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)))
            .on_action(cx.listener(|_, _: &crate::CloseWindow, window, _| window.remove_window()))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(theme::font())
            .font_weight(FontWeight::NORMAL)
            .text_size(px(14.))
            .child(self.render_title_bar(cx))
            .child(self.render_pages(cx))
            .child(
                div()
                    .id("content")
                    .track_scroll(&self.content_scroll)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(content),
            )
            .when(self.tutorial.step.is_some(), |el| el.child(self.render_tutorial(cx)))
    }
}

/// The outcome of a task, as a notification: an error stays until it is dismissed.
pub fn show_outcome(result: &Result<String, String>, window: &mut Window, cx: &mut App) {
    let note = match result {
        Ok(message) if message.is_empty() => return,
        Ok(message) => Notification::success(message.clone()),
        Err(error) => Notification::error(error.clone()).autohide(false),
    };
    let note = centered(note, window, cx);
    window.push_notification(note, cx);
}

/// Notifications sit at the bottom center of the window. With a sheet open, GPUI Kit keeps them to
/// the part of the window the sheet leaves free, a sliver in a window this narrow, so they are
/// placed from the window's left edge instead, still centered, over the sheet.
fn centered(note: Notification, window: &mut Window, cx: &mut App) -> Notification {
    if !window.has_active_sheet(cx) {
        return note;
    }
    let free = window.viewport_size().width - cx.theme().notification.width;
    Theme::global_mut(cx).notification.margins.left = (free / 2.).max(px(0.));
    note.placement(Anchor::BottomLeft)
}

fn dot(color: Hsla) -> impl IntoElement {
    div().size(px(6.)).flex_none().rounded_full().bg(color)
}

// Two Lucide icons (ISC license) that GPUI Kit's default icon set leaves out.

#[derive(Clone, Copy)]
enum Tone {
    Info,
    Warning,
    Danger,
}

/// A short note with its actions, tinted by what it is about, and a close button when it can be
/// put away. It slides in once.
fn notice(
    id: &'static str,
    tone: Tone,
    text: String,
    actions: Vec<AnyElement>,
    close: Option<AnyElement>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (icon, color) = match tone {
        Tone::Info => (Icon::new(IconName::Info), theme.info),
        Tone::Warning => (Icon::new(IconName::TriangleAlert), theme.warning),
        Tone::Danger => (Icon::new(IconName::CircleAlert), theme.danger),
    };
    h_flex()
        .id(id)
        .relative()
        .items_start()
        .gap_3()
        .px_3p5()
        .py_3()
        .rounded(theme.radius_lg)
        .bg(color.opacity(if theme.is_dark() { 0.09 } else { 0.06 }))
        .border_1()
        .border_color(color.opacity(0.22))
        .child(div().pt(px(2.)).child(icon.small().text_color(color)))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_2p5()
                .child(div().text_size(rems(0.923)).line_height(rems(1.3)).child(text))
                .when(!actions.is_empty(), |el| el.child(h_flex().gap_2().children(actions))),
        )
        .when_some(close, |el, close| el.child(div().mt(px(-3.)).mr(px(-6.)).child(close)))
        .into_any_element()
}

fn account_email(a: &Account) -> String {
    a.email.clone().unwrap_or_else(|| tf("account.unnamed", &[("id", &cc_same_core::short(&a.id))]))
}
fn desktop_usage<'a>(
    store: &'a Store,
    a: &Account,
) -> (Option<cc_same_core::usage::Usage>, Option<&'a cc_same_core::cli_usage::Reading>) {
    let reading = store
        .cli_accounts
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .into_iter()
        .flatten()
        .filter(|cli| Some(&cli.email) == a.email.as_ref())
        .filter_map(|cli| store.cli_usage.readings.get(&cli.slot))
        .filter(|r| r.usage.is_some())
        .max_by(|a, b| a.usage.unwrap().at.total_cmp(&b.usage.unwrap().at));
    (
        reading.and_then(|r| r.usage).or_else(|| store.overview.as_ref().and_then(|o| o.usage.get(&a.id)).copied()),
        reading,
    )
}
fn cli_status(reading: Option<&cc_same_core::cli_usage::Reading>) -> String {
    t(match reading.map(|r| r.status()) {
        Some("Valid") => "ui.valid",
        Some("Expired") => "ui.dead",
        Some("Renews on switch") => "ui.refreshSt",
        _ => "ui.unknown",
    })
}
fn avatar(email: &str, active: bool) -> Div {
    div()
        .flex_none()
        .size(px(28.))
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .when(active, |el| el.bg(gpui_kit::rgb(0xD97757)).text_color(gpui_kit::white()))
        .child(email.chars().next().unwrap_or('—').to_uppercase().to_string())
}
fn in_use(cx: &App) -> AnyElement {
    div()
        .px_2()
        .py_1()
        .rounded_full()
        .text_size(px(12.))
        .text_color(theme::accent_text(cx))
        .bg(gpui_kit::rgb(0xD97757).opacity(if cx.theme().is_dark() { 0.18 } else { 0.14 }))
        .child(t("ui.inUse"))
        .into_any_element()
}
#[allow(clippy::too_many_arguments)]
fn account_row(
    email: &str,
    plan: String,
    active: bool,
    usage: Option<cc_same_core::usage::Usage>,
    reading: Option<&cc_same_core::cli_usage::Reading>,
    status: String,
    attention: bool,
    action: AnyElement,
    cx: &App,
) -> AnyElement {
    h_flex()
        .h(px(54.))
        .gap_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(avatar(email, active).when(!active, |el| el.bg(theme::empty(cx))))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().child(email.to_owned()))
                        .child(div().text_size(px(12.5)).text_color(cx.theme().muted_foreground).child(plan)),
                ),
        )
        .child(usage_cell(usage.and_then(|u| u.five_hour), reading, 0, cx))
        .child(usage_cell(usage.and_then(|u| u.weekly), reading, 1, cx))
        .child(
            div()
                .w(px(96.))
                .text_size(px(12.))
                .text_color(if attention { theme::accent_text(cx) } else { cx.theme().muted_foreground })
                .child(status),
        )
        .child(h_flex().w(px(90.)).justify_end().child(action))
        .into_any_element()
}
fn usage_cell(
    value: Option<f64>,
    reading: Option<&cc_same_core::cli_usage::Reading>,
    index: usize,
    cx: &App,
) -> AnyElement {
    let hot = value.is_some_and(|v| v >= 80.);
    let color = if hot { gpui_kit::rgb(0xD97757).into() } else { theme::bar(cx) };
    let blocks = (0..10).map(|i| {
        div().w(px(6.)).h(px(10.)).rounded(px(1.)).bg(if value.is_some_and(|v| (i as f64) < (v / 10.).round()) {
            color
        } else {
            theme::empty(cx)
        })
    });
    let reset = reading
        .map(|r| r.reset_label(index))
        .filter(|s| s != "—")
        .map(|s| {
            // The core's existing local-time formatter supplies the time; localize its date suffixes here.
            let time = s.trim_end_matches(" 重置").replace('月', "/").replace('日', "");
            tf("ui.resets", &[("x", &time)])
        })
        .unwrap_or_else(|| if value.is_none() { t("ui.noReading") } else { "—".into() });
    v_flex()
        .w(px(112.))
        .gap_1()
        .child(
            h_flex().gap_1().child(h_flex().gap(px(2.)).children(blocks)).child(
                div()
                    .text_size(px(12.))
                    .text_color(if hot { theme::accent_text(cx) } else { cx.theme().foreground })
                    .child(value.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "—".into())),
            ),
        )
        .child(div().text_size(px(11.5)).text_color(cx.theme().muted_foreground).child(reset))
        .into_any_element()
}
