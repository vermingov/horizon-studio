// The opt-in Roblox account (hstudio_rbx_auth): the credential is kept out
// of everything scripts can touch, goes only to Roblox's own hosts, and is
// never forwarded to the CDN a download is redirected to. A local stand-in
// server stands in for Roblox's endpoints so the whole path -- validate,
// download with the cookie, the CDN redirect without it, caching, the
// errors, and Quick Login's CSRF retry -- is exercised without a real
// account.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio_rbx_auth::{self as auth, Account, AuthError, Hosts, QuickStatus};

/// The cookie the stand-in accepts. A real `.ROBLOSECURITY` is far longer;
/// this is only a token the server checks for.
const GOOD_COOKIE: &str = "_|WARNING:-DO-NOT-SHARE-THIS.|_test-session-cookie-value-1234567890";

/// A tiny but valid model file (one Folder named "Interface").
const MODEL: &str = r#"<roblox version="4"><Item class="Folder" referent="RBX0"><Properties><string name="Name">Interface</string></Properties></Item></roblox>"#;

/// What the stand-in saw, so a test can prove the cookie never reached the
/// CDN.
#[derive(Default)]
struct Seen {
    cookie_to_cdn: bool,
    cdn_hits: u32,
    /// A cookie turned up on a script's own `request` (it never should).
    cookie_on_script_request: bool,
    script_requests: u32,
}

/// Roblox's hosts, stood in for by one local server. It serves until the
/// process ends. `Seen` records whether the cookie leaked to the CDN.
fn stand_in() -> (Hosts, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");
    let seen = Arc::new(Mutex::new(Seen::default()));
    let seen2 = seen.clone();
    let base2 = base.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = seen2.clone();
            let base = base2.clone();
            std::thread::spawn(move || handle(stream, &base, &seen));
        }
    });
    let hosts = Hosts {
        delivery: base.clone(),
        users: base.clone(),
        tokens: base.clone(),
        auth: base,
        local: true,
    };
    (hosts, seen)
}

fn handle(stream: std::net::TcpStream, base: &str, seen: &Mutex<Seen>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut len = 0usize;
    let mut cookie = None;
    let mut csrf = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        let low = line.to_ascii_lowercase();
        if let Some(n) = low.strip_prefix("content-length:") {
            len = n.trim().parse().unwrap_or(0);
        }
        // Keep the value's case (the lowercased line is only for the name).
        if low.starts_with("cookie:") {
            cookie = Some(line[line.find(':').unwrap() + 1..].trim().to_owned());
        }
        if low.starts_with("x-csrf-token:") {
            csrf = Some(line[line.find(':').unwrap() + 1..].trim().to_owned());
        }
        if line == "\r\n" {
            break;
        }
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).unwrap();
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let has_good = cookie
        .as_deref()
        .is_some_and(|c| c.contains(".ROBLOSECURITY=") && c.contains("test-session-cookie-value"));
    let mut stream = stream;
    let send = |stream: &mut std::net::TcpStream, status: &str, headers: &str, body: &[u8]| {
        write!(
            stream,
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    };
    // Asset delivery: needs the cookie, then 302s to the CDN.
    if path.starts_with("/v1/asset/") {
        if !has_good {
            send(&mut stream, "401 Unauthorized", "", br#"{"errors":[{"code":401}]}"#);
            return;
        }
        let loc = format!("Location: {base}/cdn/model\r\n");
        send(&mut stream, "302 Found", &loc, b"");
        return;
    }
    // The CDN: the cookie must NOT be here.
    if path.starts_with("/cdn/") {
        let mut s = seen.lock().unwrap();
        s.cdn_hits += 1;
        if cookie.is_some() {
            s.cookie_to_cdn = true;
        }
        drop(s);
        send(&mut stream, "200 OK", "", MODEL.as_bytes());
        return;
    }
    // Who the cookie belongs to.
    if path.starts_with("/v1/users/authenticated") {
        if !has_good {
            send(&mut stream, "401 Unauthorized", "", br#"{"errors":[{"code":401}]}"#);
            return;
        }
        let who = br#"{"id":12345,"name":"TestUser","displayName":"Test User"}"#;
        send(&mut stream, "200 OK", "", who);
        return;
    }
    // Quick Login create.
    if path.ends_with("/login/create") {
        let body = br#"{"code":"ABCD12","status":"Created","privateKey":"priv-key-xyz","expirationTime":"2030-01-01T00:00:00Z","imagePath":"/x"}"#;
        send(&mut stream, "200 OK", "", body);
        return;
    }
    // Quick Login status: CSRF-gated (first call 403 with the token).
    if path.ends_with("/login/status") {
        if csrf.as_deref() != Some("csrf-token-1") {
            send(
                &mut stream,
                "403 Forbidden",
                "x-csrf-token: csrf-token-1\r\n",
                br#"{"errors":[{"code":0,"message":"XSRF token invalid"}]}"#,
            );
            return;
        }
        send(
            &mut stream,
            "200 OK",
            "",
            br#"{"status":"Validated","accountName":"TestUser"}"#,
        );
        return;
    }
    // Quick Login completion: CSRF-gated, answers with the session cookie.
    if path.ends_with("/v2/login") {
        if csrf.as_deref() != Some("csrf-token-1") {
            send(
                &mut stream,
                "403 Forbidden",
                "x-csrf-token: csrf-token-1\r\n",
                br#"{"message":"Token Validation Failed"}"#,
            );
            return;
        }
        // As Roblox answers: several cookies, the session one not first, with
        // Expires dates (which have commas in them).
        let set = format!(
            "Set-Cookie: RBXEventTrackerV2=CreateDate=11/2/2026; expires=Fri, 19-Mar-2027 20:00:00 GMT; domain=roblox.com; path=/\r\n\
             Set-Cookie: GuestData=UserID=-1; expires=Fri, 19-Mar-2027 20:00:00 GMT; domain=roblox.com; path=/\r\n\
             Set-Cookie: .ROBLOSECURITY={GOOD_COOKIE}; domain=.roblox.com; expires=Tue, 02-Nov-2027 20:00:00 GMT; path=/; secure; HttpOnly\r\n"
        );
        send(&mut stream, "200 OK", &set, br#"{"user":{"id":12345}}"#);
        return;
    }
    // A script's own request (to prove the cookie is never added to it).
    if path.starts_with("/echo-headers") {
        let mut s = seen.lock().unwrap();
        s.script_requests += 1;
        if cookie.as_deref().is_some_and(|c| c.contains("test-session-cookie")) {
            s.cookie_on_script_request = true;
        }
        drop(s);
        send(&mut stream, "200 OK", "", b"ok");
        return;
    }
    send(&mut stream, "404 Not Found", "", b"");
}

/// Only one test at a time may point the global hosts at a stand-in.
static HOSTS_LOCK: Mutex<()> = Mutex::new(());

/// Drives frames with real time passing (so background downloads/requests
/// finish) until `done`, or a timeout.
fn drive_until(
    st: &mut horizon_studio::game::hstudio::Studio,
    game: &mut FakeGame,
    done: impl Fn(&horizon_studio::game::hstudio::Studio) -> bool,
) {
    for _ in 0..300 {
        game.ticks += 1;
        st.frame(game, 0.05);
        if done(st) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn has_line(st: &horizon_studio::game::hstudio::Studio, tag: &str) -> bool {
    log(st)
        .iter()
        .any(|l| l.strip_prefix("[game] ").unwrap_or(l).starts_with(tag))
}

// --------------------------------------------------------------- pure calls

#[test]
fn authenticated_reads_the_account() {
    let (hosts, _) = stand_in();
    let acct = auth::authenticated(&hosts, GOOD_COOKIE).unwrap();
    assert_eq!(
        acct,
        Account {
            id: 12345,
            name: "TestUser".into(),
            display_name: "Test User".into(),
        }
    );
    // A wrong cookie is a bad credential, not a crash.
    assert_eq!(
        auth::authenticated(&hosts, "nope").unwrap_err(),
        AuthError::BadCredential
    );
}

#[test]
fn download_follows_the_redirect_without_the_cookie() {
    let (hosts, seen) = stand_in();
    let bytes = auth::download_asset(&hosts, 11389137937, GOOD_COOKIE).unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), MODEL);
    let s = seen.lock().unwrap();
    assert_eq!(s.cdn_hits, 1, "the CDN redirect was followed");
    assert!(!s.cookie_to_cdn, "the cookie must never reach the CDN");
}

#[test]
fn a_bad_cookie_download_is_a_clear_error() {
    let (hosts, _) = stand_in();
    assert_eq!(
        auth::download_asset(&hosts, 11389137937, "bad").unwrap_err(),
        AuthError::BadCredential
    );
}

#[test]
fn quick_login_runs_with_csrf_retry() {
    let (hosts, _) = stand_in();
    let code = auth::quick_create(&hosts).unwrap();
    assert_eq!(code.code, "ABCD12");
    assert!(code.enter_at.contains("crossdevicelogin"));
    // The status call is CSRF-gated: it must retry with the handed-back token.
    assert_eq!(
        auth::quick_status(&hosts, &code).unwrap(),
        QuickStatus::Validated(Some("TestUser".into()))
    );
    // Completion returns the session cookie from Set-Cookie.
    let cookie = auth::quick_complete(&hosts, &code).unwrap();
    assert_eq!(cookie, GOOD_COOKIE);
    // And that cookie signs in.
    assert_eq!(auth::authenticated(&hosts, &cookie).unwrap().name, "TestUser");
}

// --------------------------------------------------------------- storage

#[test]
fn cookie_is_stored_outside_the_workspace() {
    let mut game = FakeGame::new("auth-store");
    let st = studio(&mut game);
    let dir = st.s.dir.clone();
    auth::store_cookie(&dir, GOOD_COOKIE).unwrap();
    assert!(auth::connected(&dir));
    // It is in the studio folder, not the workspace scripts can read.
    assert!(dir.join(auth::COOKIE_FILE).is_file());
    assert!(!dir.join("workspace").join(auth::COOKIE_FILE).exists());
    assert_eq!(auth::load_cookie(&dir).as_deref(), Some(GOOD_COOKIE));
    auth::clear_cookie(&dir);
    assert!(!auth::connected(&dir));
}

// --------------------------------------------------------------- end to end

#[test]
fn get_objects_downloads_with_the_connected_account() {
    let _guard = HOSTS_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (hosts, seen) = stand_in();
    auth::set_hosts(Some(hosts));
    let mut game = FakeGame::new("auth-getobjects");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    auth::store_cookie(&st.s.dir, GOOD_COOKIE).unwrap();
    st.run_console(
        &mut game,
        r#"
        task.spawn(function()
            local objs = game:GetObjects("rbxassetid://11389137937")
            print("DL", #objs, objs[1].Name)
        end)
        "#,
    );
    drive_until(&mut st, &mut game, |st| has_line(st, "DL"));
    auth::set_hosts(None);
    let lines: Vec<String> = log(&st)
        .iter()
        .filter_map(|l| {
            let l = l.strip_prefix("[game] ").unwrap_or(l);
            l.starts_with("DL").then(|| l.to_owned())
        })
        .collect();
    assert_eq!(lines, vec!["DL    1    Interface".to_string()], "{:?}", log(&st));
    // Cached in the workspace for next time.
    let cached =
        st.s.dir
            .join("workspace")
            .join(".roblox-assets")
            .join("11389137937.rbxm");
    assert!(cached.is_file(), "the asset is cached in the workspace");
    // The cookie went to delivery, never to the CDN.
    assert!(!seen.lock().unwrap().cookie_to_cdn);
}

#[test]
fn no_account_raises_the_prompt_then_the_paste_lets_the_script_through() {
    use horizon_studio::game::hstudio_ui::{self as ui, RobloxAction};
    let _guard = HOSTS_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (hosts, _) = stand_in();
    auth::set_hosts(Some(hosts));
    auth::set_session_cookie(None);
    let mut game = FakeGame::new("auth-prompt");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    // A script asks for an uncached asset with no account connected.
    st.run_console(
        &mut game,
        r#"
        task.spawn(function()
            local objs = game:GetObjects("rbxassetid://11389137937")
            print("PR", #objs, objs[1].Name)
        end)
        "#,
    );
    run_for(&mut st, &mut game, 0.1);
    // The in-game prompt is raised (not something a script drew), and a
    // thread waits.
    assert!(st.s.ui.roblox.open, "the prompt opened");
    assert!(!st.s.ui.roblox.from_settings, "a script raised it");
    assert!(st.s.rbx.assets.iter().any(|a| a.job.is_none()));
    // The player pastes a cookie and chooses "Use once".
    st.s.ui.roblox.field.set(GOOD_COOKIE);
    ui::roblox_action(&mut st, RobloxAction::UseOnce);
    // Validation, then the download, then the waiting thread resumes.
    drive_until(&mut st, &mut game, |st| has_line(st, "PR"));
    auth::set_hosts(None);
    auth::set_session_cookie(None);
    assert!(has_line(&st, "PR"), "{:?}", log(&st));
    assert!(!st.s.ui.roblox.open, "the prompt closed on connect");
}

#[test]
fn cancelling_the_prompt_errors_the_script() {
    use horizon_studio::game::hstudio_ui::{self as ui, RobloxAction};
    let _guard = HOSTS_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (hosts, _) = stand_in();
    auth::set_hosts(Some(hosts));
    auth::set_session_cookie(None);
    let mut game = FakeGame::new("auth-cancel");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    st.run_console(
        &mut game,
        r#"
        task.spawn(function()
            local ok, err = pcall(game.GetObjects, game, "rbxassetid://11389137937")
            print("CX", ok, err)
        end)
        "#,
    );
    run_for(&mut st, &mut game, 0.1);
    assert!(st.s.ui.roblox.open);
    ui::roblox_action(&mut st, RobloxAction::Cancel);
    run_for(&mut st, &mut game, 0.2);
    auth::set_hosts(None);
    let cx = log(&st)
        .into_iter()
        .map(|l| l.strip_prefix("[game] ").unwrap_or(&l).to_owned())
        .find(|l| l.starts_with("CX"))
        .unwrap_or_default();
    assert!(
        cx.contains("false") && cx.to_lowercase().contains("cancelled"),
        "{cx:?}"
    );
}

#[test]
fn a_connected_account_is_not_handed_to_scripts() {
    let _guard = HOSTS_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (hosts, seen) = stand_in();
    let script_server = hosts.users.clone();
    auth::set_hosts(Some(hosts));
    let mut game = FakeGame::new("auth-sandbox");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    auth::store_cookie(&st.s.dir, GOOD_COOKIE).unwrap();
    // A script can neither read the credential file nor see it listed, and
    // getgenv must not hold it.
    st.run_console(
        &mut game,
        &format!(
            r#"
        print("SB read", pcall(function() return readfile("../{file}") end))
        print("SB read2", pcall(function() return readfile("{file}") end))
        local names = {{}}
        for _, f in ipairs(listfiles(".")) do names[#names+1] = f end
        print("SB list", table.concat(names, ","):find("{file}") ~= nil)
        local g = getgenv()
        local leaked = false
        for k, v in pairs(g) do
            if type(v) == "string" and v:find("test%-session%-cookie") then leaked = true end
        end
        print("SB genv", leaked)
        -- A script's own request can't even reach the studio's own hosts
        -- (they're on this machine, which scripts are firewalled from), and
        -- the request carries no cookie either way.
        task.spawn(function()
            local r = request({{ Url = "{server}/echo-headers", Method = "GET" }})
            print("SB req", r.StatusCode)
        end)
        "#,
            file = auth::COOKIE_FILE,
            server = script_server,
        ),
    );
    drive_until(&mut st, &mut game, |st| has_line(st, "SB req"));
    auth::set_hosts(None);
    // Whitespace between fields is normalized to single spaces.
    let lines: Vec<String> = log(&st)
        .iter()
        .map(|l| {
            l.strip_prefix("[game] ")
                .unwrap_or(l)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|l| l.starts_with("SB "))
        .collect();
    // Both reads fail (outside the sandbox / not in the workspace); not listed; genv clean.
    assert!(lines.iter().any(|l| l.starts_with("SB read false")), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("SB read2 false")), "{lines:?}");
    assert!(lines.contains(&"SB list false".to_string()), "{lines:?}");
    assert!(lines.contains(&"SB genv false".to_string()), "{lines:?}");
    // The script's request was refused (status 0: a studio host is on this
    // machine, which scripts may not reach), so no cookie could be sent; and
    // nothing in the studio adds the stored cookie to a script's request.
    assert!(lines.contains(&"SB req 0".to_string()), "{lines:?}");
    let s = seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(s.script_requests, 0, "a script can't reach the studio's hosts");
    assert!(!s.cookie_on_script_request);
}
