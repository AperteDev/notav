//! notav — debugging/scripting front-end for notav-core.
//! Account comes from NOTAV_URL / NOTAV_USER / NOTAV_PASS (or a .env file in the cwd).

use notav_core::Client;
use std::process::exit;

const USAGE: &str = "usage: notav <command>
  whoami                        show the account and its Files root
  sync                          push + pull everything
  login <server>                sign in with the browser (Login Flow v2), print the credentials";

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
    client.set_account(env("NOTAV_URL")?, env("NOTAV_USER")?, env("NOTAV_PASS")?)?;

    match args.first().map(String::as_str) {
        Some("whoami") => {
            let a = client.account().ok_or("no account")?;
            println!("{} at {}", a.user, a.url);
            println!("{}", client.files_home()?);
        }
        Some("sync") => client.sync()?,
        _ => return Err(USAGE.into()),
    }
    Ok(())
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
