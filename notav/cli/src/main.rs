//! notav — debugging/scripting front-end for notav-core.
//! Account comes from NOTAV_URL / NOTAV_USER / NOTAV_PASS (or a .env file in the cwd).

use notav_core::{AccountKind, Client, Note};
use std::io::Read;
use std::process::exit;

const USAGE: &str = "usage: notav <command>
  ls                            show notes (* changed here, not uploaded yet)
  cat <path>                    print a note
  new <path> [text | -]         add a note (- reads stdin)
  put <path> <text | ->         replace a note's text
  mv <path> <new path>          rename or move a note
  rm <path>                     delete a note
  mkdir <folder>                add a folder
  folder [<folder> [--move]]    show or set the notes folder (--move: take the notes along)
  sync                          push + pull everything
  whoami                        show the account and its Files root
  login <server>                sign in with the browser (Login Flow v2), print the credentials
Paths are relative to the notes folder: \"Work/Plan.md\"; changes are synced at once.";

fn main() {
    load_dotenv();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let arg = |i: usize| args.get(i).cloned().ok_or(USAGE);
    if args.first().map(String::as_str) == Some("login") {
        let flow = notav_core::login_flow_start(arg(1)?)?;
        println!("Open {}", flow.login_url);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if let Some(c) = notav_core::login_flow_poll(&flow)? {
                println!(
                    "NOTAV_URL={}\nNOTAV_USER={}\nNOTAV_PASS={}",
                    c.server, c.login_name, c.app_password
                );
                return Ok(());
            }
        }
    }

    let data = std::env::var("NOTAV_DATA").unwrap_or_else(|_| {
        let base = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            format!("{}/.local/share", std::env::var("HOME").unwrap_or_default())
        });
        format!("{base}/notav-cli")
    });
    let client = Client::open(data)?;
    let env = |k: &str| std::env::var(k).map_err(|_| format!("{k} not set"));
    let (url, user) = (env("NOTAV_URL")?, env("NOTAV_USER")?);
    // The account of these credentials, added the first time; its notes folder Nextcloud
    // Notes' or the default until set.
    let id = match client
        .accounts()
        .into_iter()
        .find(|a| a.url == url.trim_end_matches('/') && a.user == user)
    {
        Some(a) => a.id,
        None => client.add_account(AccountKind::Nextcloud, url, user, String::new())?,
    };
    client.connect(id, env("NOTAV_PASS")?)?;
    if client.chosen_notes_folder(id).is_none() {
        client.set_notes_folder(id, client.suggested_notes_folder(id), false)?;
    }

    match args.first().map(String::as_str) {
        Some("whoami") => {
            let a = client.account(id).ok_or("no account")?;
            println!("{} at {}", a.user, a.url);
            println!("{}", client.files_home(id)?);
        }
        Some("ls") => {
            for n in client.notes() {
                println!("{} {}", if n.pending { "*" } else { " " }, n.path);
            }
        }
        Some("cat") => print!("{}", text(&client, &find(&client, &arg(1)?)?)),
        Some("new") => {
            let (folder, title) = folder_title(&arg(1)?);
            let body = match args.get(2).map(String::as_str) {
                Some("-") => stdin()?,
                Some(t) => t.to_string(),
                None => String::new(),
            };
            println!("{}", client.add_note(id, folder, title, body)?.path);
            println!("{:?}", client.sync()?);
        }
        Some("put") => {
            let n = find(&client, &arg(1)?)?;
            let body = match arg(2)?.as_str() {
                "-" => stdin()?,
                t => t.to_string(),
            };
            client.set_text(n.id, body)?;
            println!("{:?}", client.sync()?);
        }
        Some("mv") => {
            let n = find(&client, &arg(1)?)?;
            let (folder, title) = folder_title(&arg(2)?);
            println!("{}", client.move_note(n.id, folder, title)?.path);
            println!("{:?}", client.sync()?);
        }
        Some("rm") => {
            client.delete_note(find(&client, &arg(1)?)?.id)?;
            println!("{:?}", client.sync()?);
        }
        Some("mkdir") => {
            client.add_folder(id, arg(1)?)?;
            println!("{:?}", client.sync()?);
        }
        Some("folder") => match args.get(1) {
            Some(f) => {
                client.set_notes_folder(id, f.clone(), args.iter().any(|a| a == "--move"))?;
                println!("{:?}", client.sync()?);
            }
            None => println!("{}", client.notes_folder(id)),
        },
        Some("sync") => println!("{:?}", client.sync()?),
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn find(client: &Client, path: &str) -> Result<Note, String> {
    client
        .notes()
        .into_iter()
        .find(|n| n.path == path || n.path == format!("{path}.md"))
        .ok_or_else(|| format!("no note '{path}' (run `notav sync` first?)"))
}

fn text(client: &Client, n: &Note) -> String {
    client.note_text(n.id).unwrap_or_default()
}

/// "Work/Plan.md" → ("Work", "Plan").
fn folder_title(path: &str) -> (String, String) {
    let path = path.strip_suffix(".md").unwrap_or(path);
    let (folder, title) = path.rsplit_once('/').unwrap_or(("", path));
    (folder.to_string(), title.to_string())
}

fn stdin() -> std::io::Result<String> {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    Ok(s)
}

fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else {
        return;
    };
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=')
            && !k.trim_start().starts_with('#')
            && std::env::var_os(k.trim()).is_none()
        {
            // SAFETY: single-threaded, before anything else runs.
            unsafe { std::env::set_var(k.trim(), v.trim()) };
        }
    }
}
