//! Terminal sign-in capture. Polling and Keychain writes never run on the UI thread.
use crate::store::Store;
use cc_same_core::cli_accounts::{self, CurrentLogin};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
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
    window.open_sheet(cx, move |sheet, _, _| {
        sheet
            .title(if target.is_some() { "Sign in again" } else { "Add CLI account" })
            .size(px(360.))
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
                                return Err("Terminal login changed; review the detected account and try again".into());
                            }
                            match target {
                                Some(slot) => cli_accounts::update_slot(&home, slot).map(|_| ()),
                                None => cli_accounts::add_current(&home).and_then(|result| match result {
                                    cli_accounts::AddResult::Added(_) => Ok(()),
                                    cli_accounts::AddResult::Existing(slot) => {
                                        anyhow::bail!("Already saved in slot {slot}; use Update login")
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
            Some(_) if !target_matches => format!(
                "This is not {} in the saved organization. Sign in to that account first.",
                target_account.map(|a| a.email.as_str()).unwrap_or("the selected account")
            ),
            Some(login) if !login.credential_exists => format!("{} — waiting for terminal credentials…", login.email),
            Some(login) => format!(
                "Detected: {} · {}{}",
                login.email,
                login.organization,
                login.existing_slot.map(|s| format!(" · Already in slot {s}")).unwrap_or_default()
            ),
            None => "Waiting for terminal login…".into(),
        };
        let can_save = target_matches
            && self.detected.as_ref().is_some_and(|l| l.credential_exists)
            && !self.saving
            && !self.saved;
        let updating = self.target.is_some() || self.detected.as_ref().is_some_and(|l| l.existing_slot.is_some());
        v_flex()
            .p_4()
            .gap_4()
            .child(
                div().child(
                    target_account
                        .map(|a| format!("1. In Terminal, sign in as {} in {}.", a.email, a.organization))
                        .unwrap_or_else(|| "1. Sign in in Terminal with the account you want to save.".into()),
                ),
            )
            .child(div().child("claude auth login"))
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("copy-cli-login").small().label("Copy").on_click(|_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string("claude auth login".into()))
                    }))
                    .child(Button::new("open-cli-terminal").small().label("Open Terminal").on_click(cx.listener(
                        |_, _, _, cx| {
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
                                        this.error = Some(
                                            "Could not open Terminal. Open it manually and run claude auth login."
                                                .into(),
                                        );
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        },
                    ))),
            )
            .child(div().child("2. The terminal login is checked every 2 seconds."))
            .child(div().child(detected))
            .children(self.poll_error.clone().map(|error| div().child(error)))
            .children(self.error.clone().map(|error| div().child(error)))
            .children(self.saved.then(|| div().child("Login saved. Desktop is unchanged.")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("cancel-cli-login")
                            .label(if self.saved { "Done" } else { "Cancel" })
                            .disabled(self.saving)
                            .on_click(|_, window, cx| window.close_sheet(cx)),
                    )
                    .child(
                        Button::new("save-cli-login")
                            .primary()
                            .label(if self.saving {
                                "Saving…"
                            } else if updating {
                                "Update login"
                            } else {
                                "Save account"
                            })
                            .disabled(!can_save)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ),
            )
    }
}
