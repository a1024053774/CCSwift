//! Terminal sign-in capture. Polling and Keychain writes never run on the UI thread.
use crate::i18n::{t, tf};
use crate::store::Store;
use crate::theme;
use cc_same_core::cli_accounts::{self, CurrentLogin};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, ClipboardItem, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div, px,
};
use std::{path::PathBuf, time::Duration};

pub fn open(store: Entity<Store>, target: Option<u64>, window: &mut Window, cx: &mut App) {
    let dialog = cx.new(|cx| {
        let mut dialog =
            CliLogin { store, target, detected: None, error: None, poll_error: None, saving: false, saved: false };
        dialog.poll(cx);
        dialog
    });
    window.open_dialog(cx, move |sheet, _, _| {
        sheet
            .title(crate::theme::dialog_title(t(if target.is_some() { "ui.relogin" } else { "capture.title" })))
            .width(px(400.))
            .child(dialog.clone())
    });
}

struct CliLogin {
    store: Entity<Store>,
    target: Option<u64>,
    detected: Option<CurrentLogin>,
    error: Option<String>,
    poll_error: Option<String>,
    saving: bool,
    saved: bool,
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or("HOME is not set".into())
}

impl CliLogin {
    fn poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let result = cx
                    .background_executor()
                    .spawn(async {
                        home().and_then(|home| cli_accounts::current_login(&home).map_err(|e| format!("{e:#}")))
                    })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if !this.saving && !this.saved {
                            match result {
                                Ok(login) => {
                                    this.detected = login;
                                    this.poll_error = None;
                                }
                                Err(error) => {
                                    this.detected = None;
                                    this.poll_error = Some(error);
                                }
                            }
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(2)).await;
            }
        })
        .detach();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.saved {
            return;
        }
        let Some(login) = self.detected.clone().filter(|l| l.credential_exists) else {
            return;
        };
        let target = self.target.or(login.existing_slot);
        let store = self.store.clone();
        self.saving = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result =
                cx.background_executor()
                    .spawn(async move {
                        home().and_then(|home| {
                            // The polling result is only a preview. Recheck before saving, and the core
                            // checks the targeted slot's identity again while holding the store locks.
                            let current = cli_accounts::current_login(&home).map_err(|e| format!("{e:#}"))?;
                            if current.as_ref().is_none_or(|c| {
                                c.email != login.email || c.organization_uuid != login.organization_uuid
                            }) {
                                return Err(t("capture.changed"));
                            }
                            match target {
                                Some(slot) => cli_accounts::update_slot(&home, slot).map(|_| ()),
                                None => cli_accounts::add_current(&home).and_then(|result| match result {
                                    cli_accounts::AddResult::Added(_) => Ok(()),
                                    cli_accounts::AddResult::Existing(slot) => {
                                        anyhow::bail!("{}", tf("capture.already", &[("slot", &slot)]))
                                    }
                                }),
                            }
                            .map_err(|e| format!("{e:#}"))
                        })
                    })
                    .await;
            if result.is_ok() {
                store.update(cx, |store, cx| store.invalidate_cli_usage(cx));
            }
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.saved = true;
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for CliLogin {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target_account = self
            .store
            .read(cx)
            .cli_accounts
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|accounts| accounts.iter().find(|a| Some(a.slot) == self.target));
        // Duplicate slots may share an identity; the core is the final authority on org matching.
        let target_matches = self.target.is_none()
            || target_account.is_some_and(|a| {
                self.detected.as_ref().is_some_and(|l| l.email == a.email && l.organization_uuid == a.organization_uuid)
            });
        let detected = match &self.detected {
            Some(_) if !target_matches => {
                tf("capture.mismatch", &[("email", &target_account.map(|a| a.email.as_str()).unwrap_or("—"))])
            }
            Some(login) if !login.credential_exists => tf("capture.credentials", &[("email", &login.email)]),
            Some(login) => format!(
                "{}{}",
                tf("capture.detected", &[("email", &login.email), ("org", &login.organization)]),
                login
                    .existing_slot
                    .map(|slot| format!(" · {}", tf("capture.existing", &[("slot", &slot)])))
                    .unwrap_or_default()
            ),
            None => t("capture.waiting"),
        };
        let can_save = target_matches
            && self.detected.as_ref().is_some_and(|l| l.credential_exists)
            && !self.saving
            && !self.saved;
        let updating = self.target.is_some() || self.detected.as_ref().is_some_and(|l| l.existing_slot.is_some());
        v_flex()
            .font_family(theme::font())
            .text_size(px(14.))
            .gap_4()
            .child(
                div().child(
                    target_account
                        .map(|a| tf("capture.updateIntro", &[("email", &a.email), ("org", &a.organization)]))
                        .unwrap_or_else(|| t("capture.intro")),
                ),
            )
            .child(
                div()
                    .px_3()
                    .py_3()
                    .rounded(px(11.))
                    .bg(theme::jelly())
                    .text_color(gpui_kit::white())
                    .font_family("monospace")
                    .child("claude auth login"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("copy-cli-login").small().label(t("capture.copy")).on_click(|_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string("claude auth login".into()))
                    }))
                    .child(Button::new("open-cli-terminal").small().label(t("capture.terminal")).on_click(
                        cx.listener(|_, _, _, cx| {
                            cx.spawn(async move |this, cx| {
                                let opened = cx
                                    .background_executor()
                                    .spawn(async {
                                        std::process::Command::new("/usr/bin/open")
                                            .args(["-a", "Terminal"])
                                            .status()
                                            .is_ok_and(|s| s.success())
                                    })
                                    .await;
                                if !opened {
                                    let _ = this.update(cx, |this, cx| {
                                        this.error = Some(t("capture.terminalFailed"));
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        }),
                    )),
            )
            .child(div().text_size(px(12.)).text_color(cx.theme().muted_foreground).child(t("capture.poll")))
            .child(div().p_3().rounded(px(10.)).bg(cx.theme().muted).child(detected))
            .children(self.poll_error.clone().map(|error| div().child(error)))
            .children(self.error.clone().map(|error| div().child(error)))
            .children(self.saved.then(|| div().child(t("capture.saved"))))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("cancel-cli-login")
                            .label(t(if self.saved { "capture.done" } else { "dialog.cancel" }))
                            .disabled(self.saving)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("save-cli-login")
                            .primary()
                            .bg(theme::jelly())
                            .label(t(if self.saving {
                                "capture.saving"
                            } else if updating {
                                "capture.update"
                            } else {
                                "capture.save"
                            }))
                            .disabled(!can_save)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ),
            )
    }
}
