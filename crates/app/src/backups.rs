//! Desktop snapshot dialog. Disk enumeration and size calculation run in Store's worker.
use crate::{
    i18n::{t, tf},
    model,
    store::Store,
    theme,
};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{App, Entity, InteractiveElement as _, ParentElement as _, SharedString, Styled as _, Window, div, px};

pub fn open(store: Entity<Store>, view: Entity<crate::view::UniApp>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let state = store.read(cx);
        let idle = state.busy.is_none();
        let cfg = state.config();
        let mut rows = v_flex().id("snapshot-list").max_h(px(300.)).overflow_y_scrollbar();
        for snapshot in state.snapshots.iter().rev() {
            let snap = snapshot.clone();
            let path = state.ctx.paths.snapshots_dir().join(&snap.id);
            let accounts = snap.partitions.iter().map(|p| &p.acct).collect::<std::collections::BTreeSet<_>>().len();
            let size = state
                .snapshot_sizes
                .get(&snap.id)
                .copied()
                .flatten()
                .map(|bytes| format!("{:.1} MB", bytes as f64 / 1_048_576.))
                .unwrap_or_else(|| "—".into());
            let why = model::snapshot_reason(
                &snap.reason,
                state.overview.as_ref().and_then(|o| o.baseline.as_deref()) == Some(snap.id.as_str()),
            );
            let restoring = view.clone();
            rows = rows.child(
                columns(
                    v_flex().child(model::snapshot_when(&snap.id)).child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(tf("backup.accounts", &[("n", &accounts)])),
                    ),
                    div().text_size(px(12.)).child(why),
                    div().text_size(px(12.)).text_color(cx.theme().muted_foreground).child(size),
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new(SharedString::from(format!("reveal-{}", snap.id)))
                                .small()
                                .outline()
                                .icon(gpui_kit::component::IconName::FolderOpen)
                                .tooltip(t("backup.reveal"))
                                .on_click(move |_, _, cx| cx.reveal_path(&path)),
                        )
                        .child(
                            Button::new(SharedString::from(format!("restore-{}", snap.id)))
                                .small()
                                .outline()
                                .label(t("settings.restore"))
                                .disabled(!idle)
                                .on_click(move |_, window, cx| {
                                    restoring.update(cx, |this, cx| this.confirm_restore(&snap, window, cx));
                                }),
                        ),
                )
                .h(px(52.))
                .flex_none()
                .border_b_1()
                .border_color(cx.theme().border),
            );
        }
        if state.snapshots.is_empty() {
            rows = rows.child(t("settings.no_snapshots"));
        }
        let saving = store.clone();
        let keeping = store.clone();
        dialog.width(px(600.)).title(theme::dialog_title(t("backup.title"))).child(
            v_flex()
                .font_family(theme::font())
                .gap_3()
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(cx.theme().muted_foreground)
                                .child(t("backup.subtitle")),
                        )
                        .child(
                            Button::new("backup-now")
                                .small()
                                .primary()
                                .bg(theme::jelly())
                                .label(t("backup.now"))
                                .disabled(!idle)
                                .on_click(move |_, _, cx| saving.update(cx, |s, cx| s.backup_now(cx))),
                        ),
                )
                .child(
                    columns(
                        div().child(t("backup.time")),
                        div().child(t("backup.reason")),
                        div().child(t("backup.size")),
                        div().child(t("ui.colAction")),
                    )
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground),
                )
                .child(rows)
                .child(
                    h_flex().gap_2().child(t("backup.keep")).child(
                        Button::new("keep-snapshots")
                            .small()
                            .outline()
                            .label(cfg.keep_snapshots.to_string())
                            .dropdown_menu(move |mut menu, _, _| {
                                for n in [10, 20, 50] {
                                    let store = keeping.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(n.to_string()).checked(cfg.keep_snapshots == n).on_click(
                                            move |_, _, cx| {
                                                store.update(cx, |s, cx| {
                                                    s.update_config(cx, |cfg| cfg.keep_snapshots = n)
                                                })
                                            },
                                        ),
                                    );
                                }
                                menu
                            }),
                    ),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(tf("backup.schedule", &[("minutes", &(cfg.snapshot_every_seconds / 60.))])),
                ),
        )
    });
}

/// One layout for the header and every row, so each heading sits over its values. The action
/// cell is right-aligned, like the main table's.
fn columns(
    time: impl gpui_kit::IntoElement,
    reason: impl gpui_kit::IntoElement,
    size: impl gpui_kit::IntoElement,
    action: impl gpui_kit::IntoElement,
) -> gpui_kit::Div {
    h_flex()
        .gap_3()
        .child(div().flex_1().min_w_0().child(time))
        .child(div().w(px(96.)).child(reason))
        .child(div().w(px(72.)).child(size))
        .child(h_flex().w(px(120.)).justify_end().child(action))
}

pub(crate) fn directory_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut bytes = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.path().symlink_metadata()?;
        if metadata.is_dir() {
            bytes += directory_size(&entry.path())?;
        } else if metadata.is_file() {
            bytes += metadata.len();
        }
    }
    Ok(bytes)
}
