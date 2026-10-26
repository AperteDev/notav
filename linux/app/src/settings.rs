//! Preferences dialog: General (style, the tree's density and icons, the notes folder) and
//! Accounts (account, settings sync).

use crate::connection::Connection;
use crate::core;
use crate::i18n::gettext;
use crate::window::{get, url_text, view};
use adw::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

/// Light or dark as the device-only `color_scheme` says: "light", "dark", else the system's.
pub fn apply_color_scheme() {
    let scheme = match core().setting("color_scheme".into()).as_deref() {
        Some("light") => adw::ColorScheme::ForceLight,
        Some("dark") => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let b = view!("settings");
    let dialog: adw::PreferencesDialog = get(&b, "dialog");
    connect_account(&b, &dialog);
    connect_folder(&b, &dialog);

    let sync: adw::SwitchRow = get(&b, "sync_settings");
    sync.set_active(core().setting("sync_settings".into()).as_deref() != Some("false"));
    sync.connect_active_notify(|s| {
        let _ = core().set_setting("sync_settings".into(), s.is_active().to_string());
        crate::sync_now(); // turning it on pulls the server's settings right away
    });
    let style: adw::ToggleGroup = get(&b, "color_scheme");
    let current = core().setting("color_scheme".into());
    style.set_active_name(Some(match current.as_deref() {
        Some("light") => "light",
        Some("dark") => "dark",
        _ => "system",
    }));
    style.connect_active_name_notify(|g| {
        let name = g.active_name().map(|n| n.to_string()).unwrap_or_default();
        let _ = core().set_setting("color_scheme".into(), name);
        apply_color_scheme();
    });
    let dense: adw::SwitchRow = get(&b, "dense_tree");
    dense.set_active(crate::window::dense_tree());
    dense.connect_active_notify(|s| {
        let _ = core().set_setting("dense_tree".into(), s.is_active().to_string());
        crate::window::apply_tree_look();
    });
    let icons: adw::ToggleGroup = get(&b, "tree_icons");
    let current = core().setting("tree_icons".into());
    icons.set_active_name(Some(match current.as_deref() {
        Some("all") => "all",
        Some("none") => "none",
        _ => "notes",
    }));
    icons.connect_active_name_notify(|g| {
        let name = g.active_name().map(|n| n.to_string()).unwrap_or_default();
        let _ = core().set_setting("tree_icons".into(), name);
        crate::window::apply_tree_look();
    });
    dialog.present(Some(parent));
}

/// The notes folder: another one asks whether the notes move along (when there are some), then
/// the notes there are shown.
fn connect_folder(b: &gtk::Builder, dialog: &adw::PreferencesDialog) {
    let row: adw::EntryRow = get(b, "notes_folder");
    row.set_text(&core().notes_folder());
    let d = dialog.clone();
    row.connect_apply(move |row| {
        let folder = row.text().trim().trim_matches('/').to_string();
        let old = core().notes_folder();
        if folder.is_empty() || folder == old {
            row.set_text(&old);
            return;
        }
        let (d, row) = (d.clone(), row.clone());
        glib::spawn_future_local(async move {
            let move_notes = if core().notes().is_empty() {
                false
            } else {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Move the Notes?")),
                    Some(
                        &gettext(
                            "Move your notes from “{old}” to “{new}”, or leave them there and show the notes in “{new}”?",
                        )
                        .replace("{old}", &old)
                        .replace("{new}", &folder),
                    ),
                );
                alert.add_responses(&[
                    ("cancel", &gettext("Cancel")),
                    ("keep", &gettext("Leave Them")),
                    ("move", &gettext("Move")),
                ]);
                alert.set_response_appearance("move", adw::ResponseAppearance::Suggested);
                alert.set_default_response(Some("move"));
                alert.set_close_response("cancel");
                match alert.choose_future(Some(&d)).await.as_str() {
                    "move" => true,
                    "keep" => false,
                    _ => {
                        row.set_text(&old);
                        return;
                    }
                }
            };
            crate::window::save();
            row.set_sensitive(false);
            let f = folder.clone();
            let r = gio::spawn_blocking(move || core().set_notes_folder(f, move_notes)).await;
            row.set_sensitive(true);
            let msg = match r {
                Ok(Ok(())) => {
                    crate::sync_now();
                    gettext("Notes folder changed to “{folder}”").replace("{folder}", &folder)
                }
                Ok(Err(notav_core::Error::Invalid(_))) => gettext("Not a folder name"),
                Ok(Err(e)) => gettext("Notes folder not changed: {error}")
                    .replace("{error}", &e.to_string()),
                Err(_) => gettext("Notes folder not changed"),
            };
            row.set_text(&core().notes_folder());
            d.add_toast(crate::window::note(&msg));
        });
    });
}

/// The "Account" group: a status row, then the shared GOA / server rows, applied at once.
/// Turning GOA on (or picking another account) connects through it; turning it off goes back
/// to the last browser sign-in, or disconnects without one. Signing in with the browser
/// replaces the connection (or renews that sign-in).
fn connect_account(b: &gtk::Builder, dialog: &adw::PreferencesDialog) -> Rc<Connection> {
    let goa = crate::goa_id();
    let conn = Connection::new(goa.is_some(), goa.as_deref());
    conn.attach(&get(b, "account"));
    let status: adw::ActionRow = get(b, "status");
    let icon: gtk::Image = get(b, "status_icon");
    let sign_in: adw::SplitButton = get(b, "sign_in");
    let sign_out: adw::SplitButton = get(b, "sign_out");
    // The notes folder and sync are about the account.
    let server_groups: [adw::PreferencesGroup; 2] = [get(b, "notes"), get(b, "sync")];
    // Everything here shows the actual connection, the switch included.
    let describe = {
        let (conn, sign_in, sign_out) = (conn.clone(), sign_in.clone(), sign_out.clone());
        move || {
            let goa = crate::goa_id().is_some();
            let acc = core().account().filter(|_| crate::connected());
            let host = acc.as_ref().map_or(String::new(), |a| {
                a.url
                    .split_once("://")
                    .map_or(a.url.clone(), |(_, h)| h.to_string())
            });
            match &acc {
                Some(a) if goa => {
                    status.set_title(&gettext("Connected as {name}").replace("{name}", &a.user));
                    status.set_subtitle(&format!("{host} · {}", gettext("GNOME Online Accounts")));
                }
                Some(a) => {
                    status.set_title(&gettext("Signed in as {name}").replace("{name}", &a.user));
                    status.set_subtitle(&host);
                }
                None => {
                    status.set_title(&gettext("Not signed in"));
                    status.set_subtitle("");
                }
            }
            icon.set_visible(acc.is_some());
            for g in &server_groups {
                g.set_sensitive(acc.is_some());
            }
            sign_out.set_visible(acc.is_some() && !goa);
            sign_in.set_visible(acc.is_none());
            conn.goa.set_active(goa); // its handler sees nothing to change
            conn.goa.set_sensitive(conn.has_accounts());
        }
    };
    describe();

    let (d, c, show) = (dialog.clone(), conn.clone(), describe.clone());
    conn.connect_changed(move || {
        let current = crate::goa_id();
        let chosen = c.chosen();
        if chosen.as_ref().map(|a| &a.id) == current.as_ref() {
            return; // already so
        }
        c.goa.set_sensitive(false);
        let (d, show) = (d.clone(), show.clone());
        glib::spawn_future_local(async move {
            let r = match chosen {
                Some(a) => {
                    let name = a.name.clone();
                    use_account(crate::Target::Goa(a))
                        .await
                        .map(|()| gettext("Connected as {name}").replace("{name}", &name))
                }
                None => match crate::remembered_sign_in() {
                    Some(c) => {
                        let who = c.login_name.clone();
                        use_account(crate::Target::Manual(c))
                            .await
                            .map(|()| gettext("Signed in as {name}").replace("{name}", &who))
                    }
                    None => crate::disconnect(false)
                        .await
                        .map(|()| gettext("Disconnected")),
                },
            };
            show(); // also puts the switch back when that failed
            d.add_toast(crate::window::note(&r.unwrap_or_else(|e| e)));
        });
    });

    // Back from "Waiting for …" once a sign-in ends or is cancelled.
    let cancel: gtk::Button = get(b, "cancel_sign_in");
    let reset = {
        let (sign_in, sign_out, cancel) = (sign_in.clone(), sign_out.clone(), cancel.clone());
        let labels = (sign_in.label(), sign_out.label());
        let describe = describe.clone();
        move || {
            for (button, label) in [(&sign_in, &labels.0), (&sign_out, &labels.1)] {
                button.set_sensitive(true);
                button.set_label(label.as_deref().unwrap_or_default());
            }
            cancel.set_visible(false);
            describe();
        }
    };
    let r = reset.clone();
    cancel.connect_clicked(move |_| {
        crate::window::cancel_login();
        r();
    });

    // Sign in with the browser (or by a copied link): renews the account signed in to here, or
    // replaces the GOA one.
    let start = {
        let (d, c, sign_in, sign_out) = (
            dialog.clone(),
            conn.clone(),
            sign_in.clone(),
            sign_out.clone(),
        );
        move |copy: bool| {
            let server = url_text(&c.server);
            if server.is_empty() {
                d.add_toast(crate::window::note(&crate::window::no_server()));
                return;
            }
            let button = if sign_in.is_visible() {
                &sign_in
            } else {
                &sign_out
            };
            button.set_sensitive(false);
            button.set_label(&if copy {
                gettext("Waiting for Sign-In…")
            } else {
                gettext("Waiting for Browser…")
            });
            cancel.set_visible(true);
            let (d, reset) = (d.clone(), reset.clone());
            glib::spawn_future_local(async move {
                let msg = match crate::window::browser_login(server, copy).await {
                    Ok(Some(c)) => use_sign_in(c).await,
                    Ok(None) => return, // cancelled: Cancel or closing the dialog reset it
                    Err(e) => e,
                };
                reset();
                d.add_toast(crate::window::note(&msg));
            });
        }
    };
    let actions = gio::SimpleActionGroup::new();
    for (name, copy) in [("sign-in", false), ("sign-in-copy", true)] {
        let action = gio::SimpleAction::new(name, None);
        let start = start.clone();
        action.connect_activate(move |_, _| start(copy));
        actions.add_action(&action);
    }
    // Or with a login name and app password typed in.
    let password = gio::SimpleAction::new("sign-in-password", None);
    let (d, c, describe_) = (dialog.clone(), conn.clone(), describe.clone());
    password.connect_activate(move |_, _| {
        let server = url_text(&c.server);
        if server.is_empty() {
            d.add_toast(crate::window::note(&crate::window::no_server()));
            return;
        }
        let (dd, show) = (d.clone(), describe_.clone());
        crate::window::password_login(&d, server, move |c| {
            let (d, show) = (dd.clone(), show.clone());
            glib::spawn_future_local(async move {
                let msg = use_sign_in(c).await;
                show();
                d.add_toast(crate::window::note(&msg));
            });
        });
    });
    actions.add_action(&password);
    dialog.insert_action_group("account", Some(&actions));
    sign_in.connect_clicked(move |_| start(false));
    // Only offered for a browser sign-in; a GOA account is managed in GNOME Settings.
    // The welcome page at once; pending changes still go up first, in the background.
    let d = dialog.clone();
    sign_out.connect_clicked(move |_| {
        crate::window::save();
        crate::forget_sign_in();
        d.close();
        crate::window::show_welcome();
        glib::spawn_future_local(async {
            if let Err(e) = crate::disconnect(true).await {
                crate::window::toast(&e);
            }
        });
    });
    // A sign-in still waiting for the browser stops with the dialog.
    dialog.connect_closed(|_| crate::window::cancel_login());
    conn
}

/// Connect with `target` from here: the notes folder stays as set, so syncing starts at once.
async fn use_account(target: crate::Target) -> Result<(), String> {
    crate::switch_account(target).await?;
    crate::start_background();
    crate::window::account_changed();
    Ok(())
}

/// Connect with credentials from a sign-in here; the message for a toast.
async fn use_sign_in(c: notav_core::Credentials) -> String {
    let who = c.login_name.clone();
    match use_account(crate::Target::Manual(c)).await {
        Ok(()) => gettext("Signed in as {name}").replace("{name}", &who),
        Err(e) => e,
    }
}
