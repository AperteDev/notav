//! What Nextcloud keeps besides the notes (`notav_core`'s `files.rs`): the Trash dialog, from
//! the main menu, and a note's Versions, from its menu in the tree. Each is offered only where
//! an account's server has it (`Client::features`); the server is asked in the background, the
//! dialog showing a spinner meanwhile.

use crate::core;
use crate::i18n::gettext;
use crate::window::{get, note, toast, view};
use adw::prelude::*;
use gtk::{gio, glib};

/// A date and time in the locale's format.
fn when(t: i64) -> String {
    glib::DateTime::from_unix_local(t)
        .and_then(|d| d.format("%x %X"))
        .map(|d| d.to_string())
        .unwrap_or_default()
}

/// The Trash dialog: what was deleted from the notes folders of the accounts whose server keeps
/// it, latest first, each to restore or (where the server lets it) to delete for good. Notes
/// deleted here and not synced yet go to the server first, so they are listed too.
pub fn trash() {
    let b = view!("trash");
    let dialog: adw::Dialog = get(&b, "dialog");
    let stack: gtk::Stack = get(&b, "stack");
    let list: gtk::ListBox = get(&b, "list");
    let error: adw::StatusPage = get(&b, "error");
    let toasts: adw::ToastOverlay = get(&b, "toasts");
    crate::window::present_dialog(&dialog);
    let accounts: Vec<i64> = crate::window::trash_accounts()
        .iter()
        .map(|a| a.id)
        .collect();
    // With more than one, each row says which account it is from.
    let named = accounts.len() > 1;
    glib::spawn_future_local(async move {
        let found = gio::spawn_blocking(move || {
            let _ = core().sync();
            let mut all = Vec::new();
            for a in accounts {
                all.extend(core().trash(a)?.into_iter().map(|d| (a, d)));
            }
            all.sort_by_key(|(_, d)| std::cmp::Reverse(d.deleted));
            Ok::<_, notav_core::Error>(all)
        })
        .await;
        let deleted = match found {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => {
                error.set_description(Some(&e.to_string()));
                stack.set_visible_child_name("error");
                return;
            }
            Err(_) => return,
        };
        for (account, d) in deleted {
            let from = core()
                .account(account)
                .filter(|_| named)
                .map(|a| crate::account_name(&a));
            list.append(&trash_row(account, from, &d, &list, &stack, &toasts));
        }
        show_list(&list, &stack);
    });
}

/// The list, or the empty page once nothing is left in it.
fn show_list(list: &gtk::ListBox, stack: &gtk::Stack) {
    stack.set_visible_child_name(if list.first_child().is_some() {
        "list"
    } else {
        "empty"
    });
}

/// A deleted note or folder of `account` (named `from`, with more than one).
fn trash_row(
    account: i64,
    from: Option<String>,
    d: &notav_core::Deleted,
    list: &gtk::ListBox,
    stack: &gtk::Stack,
    toasts: &adw::ToastOverlay,
) -> adw::ActionRow {
    let purge = core().features(account).purge;
    let (folder, name) = d.path.rsplit_once('/').unwrap_or(("", &d.path));
    let title = if d.folder {
        name
    } else {
        name.rsplit_once('.').map_or(name, |(t, _)| t)
    };
    let deleted = gettext("Deleted {date}").replace("{date}", &when(d.deleted));
    let subtitle = from
        .into_iter()
        .chain(Some(folder.to_string()).filter(|f| !f.is_empty()))
        .chain([deleted])
        .collect::<Vec<_>>()
        .join(" · ");
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(title))
        .subtitle(glib::markup_escape_text(&subtitle))
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(if d.folder {
        "folder-symbolic"
    } else {
        "text-x-generic-symbolic"
    }));
    let button = |icon: &str, tooltip: &str| {
        let b = gtk::Button::from_icon_name(icon);
        b.set_tooltip_text(Some(tooltip));
        b.set_valign(gtk::Align::Center);
        b.add_css_class("flat");
        row.add_suffix(&b);
        b
    };
    let restore = button("edit-undo-symbolic", &gettext("Restore"));
    let (id, name) = (d.id.clone(), title.to_string());
    let (r, l, s, t) = (row.clone(), list.clone(), stack.clone(), toasts.clone());
    restore.connect_clicked(move |b| {
        b.set_sensitive(false);
        let (id, title, r, l, s, t, b) = (
            id.clone(),
            name.clone(),
            r.clone(),
            l.clone(),
            s.clone(),
            t.clone(),
            b.clone(),
        );
        glib::spawn_future_local(async move {
            match gio::spawn_blocking(move || core().restore_deleted(account, id)).await {
                Ok(Ok(())) => {
                    l.remove(&r);
                    show_list(&l, &s);
                    crate::sync_now();
                    t.add_toast(note(
                        &gettext("“{title}” restored").replace("{title}", &title),
                    ));
                }
                Ok(Err(e)) => {
                    b.set_sensitive(true);
                    t.add_toast(note(&e.to_string()));
                }
                Err(_) => {}
            }
        });
    });
    if purge {
        let delete = button("user-trash-full-symbolic", &gettext("Delete Permanently"));
        let (id, name) = (d.id.clone(), title.to_string());
        let (r, l, s, t) = (row.clone(), list.clone(), stack.clone(), toasts.clone());
        delete.connect_clicked(move |b| {
            let (id, title, r, l, s, t, b) = (
                id.clone(),
                name.clone(),
                r.clone(),
                l.clone(),
                s.clone(),
                t.clone(),
                b.clone(),
            );
            glib::spawn_future_local(async move {
                if !confirm_purge(&r, &title).await {
                    return;
                }
                b.set_sensitive(false);
                match gio::spawn_blocking(move || core().purge(account, id)).await {
                    Ok(Ok(())) => {
                        l.remove(&r);
                        show_list(&l, &s);
                    }
                    Ok(Err(e)) => {
                        b.set_sensitive(true);
                        t.add_toast(note(&e.to_string()));
                    }
                    Err(_) => {}
                }
            });
        });
    }
    row
}

/// Ask before deleting `title` for good: nothing brings it back.
async fn confirm_purge(over: &impl IsA<gtk::Widget>, title: &str) -> bool {
    let alert = adw::AlertDialog::new(
        Some(&gettext("Delete Permanently?")),
        Some(&gettext("“{title}” cannot be restored afterwards").replace("{title}", title)),
    );
    alert.add_responses(&[
        ("cancel", &gettext("Cancel")),
        ("delete", &gettext("Delete")),
    ]);
    alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    alert.set_close_response("cancel");
    alert.choose_future(Some(over)).await == "delete"
}

/// The Versions dialog of note `id`: the versions Nextcloud keeps, latest first; one opened
/// shows its text, and Restore makes that the note's.
pub fn versions(id: i64) {
    let Some(n) = core().note(id) else {
        return;
    };
    let b = view!("versions");
    let dialog: adw::Dialog = get(&b, "dialog");
    let nav: adw::NavigationView = get(&b, "nav");
    let stack: gtk::Stack = get(&b, "stack");
    let list: gtk::ListBox = get(&b, "list");
    let error: adw::StatusPage = get(&b, "error");
    let preview: adw::NavigationPage = get(&b, "preview");
    let text: gtk::TextView = get(&b, "text");
    let restore: gtk::Button = get(&b, "restore");
    let toasts: adw::ToastOverlay = get(&b, "toasts");
    dialog.set_title(&n.title);
    crate::window::present_dialog(&dialog);
    // The version shown on the preview page: (its time, its text).
    let shown: std::rc::Rc<std::cell::RefCell<Option<(i64, String)>>> = Default::default();
    let (s, d) = (shown.clone(), dialog.clone());
    restore.connect_clicked(move |_| {
        if let Some((at, body)) = s.borrow().clone() {
            crate::window::put_text(id, &body);
            d.close();
            toast(&gettext("Restored the version of {date}").replace("{date}", &when(at)));
        }
    });
    glib::spawn_future_local(async move {
        // What waits for the server goes first, so the latest version is the note as it is.
        let account = n.account;
        let found = gio::spawn_blocking(move || {
            let _ = core().sync_account(account);
            core().versions(id)
        })
        .await;
        let versions = match found {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                error.set_description(Some(&e.to_string()));
                stack.set_visible_child_name("error");
                return;
            }
            Err(_) => return,
        };
        // Only the note as it is: nothing earlier to restore.
        if versions.iter().all(|v| v.current) {
            stack.set_visible_child_name("empty");
            return;
        }
        for v in versions {
            let mut details: Vec<String> = Vec::new();
            if let Some(label) = v.label.filter(|l| !l.is_empty()) {
                details.push(label);
            }
            if let Some(author) = v.author.filter(|a| !a.is_empty()) {
                details.push(author);
            }
            if v.current {
                details.insert(0, gettext("Current version"));
            }
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&when(v.modified)))
                .subtitle(glib::markup_escape_text(&details.join(" · ")))
                .activatable(!v.current)
                .build();
            if !v.current {
                row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                let (nav, preview, text, shown, toasts) = (
                    nav.clone(),
                    preview.clone(),
                    text.clone(),
                    shown.clone(),
                    toasts.clone(),
                );
                let (version, at, account) = (v.id.clone(), v.modified, n.account);
                row.connect_activated(move |row| {
                    row.set_sensitive(false);
                    let (nav, preview, text, shown, toasts, row, version) = (
                        nav.clone(),
                        preview.clone(),
                        text.clone(),
                        shown.clone(),
                        toasts.clone(),
                        row.clone(),
                        version.clone(),
                    );
                    glib::spawn_future_local(async move {
                        let got =
                            gio::spawn_blocking(move || core().version_text(account, version))
                                .await;
                        row.set_sensitive(true);
                        match got {
                            Ok(Ok(body)) => {
                                text.buffer().set_text(&body);
                                preview.set_title(&when(at));
                                *shown.borrow_mut() = Some((at, body));
                                nav.push(&preview);
                            }
                            Ok(Err(e)) => toasts.add_toast(note(&e.to_string())),
                            Err(_) => {}
                        }
                    });
                });
            }
            list.append(&row);
        }
        show_list(&list, &stack);
    });
}
