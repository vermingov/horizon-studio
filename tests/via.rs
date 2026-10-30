// ViaHorizon without the game: the protocol side's routing and OpenAuthMod
// answers, the manager's rules (which joins go through ViaProxy, settings,
// Java versions, archives), and the script API and the ViaHorizon mod.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_protocol::hstudio_via_net as wire;
use horizon_studio::game::hstudio::{ModState, Studio};
use horizon_studio::game::hstudio_ui::{Input, draw_screen, handle_screen};
use horizon_studio::net::hstudio_via as via;
use std::sync::{Arc, Mutex};

fn console(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    st.run_console(game, src).into_iter().map(|(_, t)| t).collect()
}

#[test]
fn internal_hosts_carry_the_server_and_version() {
    let h = wire::internal_host("play.example.org", "play.example.org:25570", "1.8.x");
    assert_eq!(h, "play.example.org\u{7}play.example.org:25570\u{7}1.8.x");
    assert!(wire::routed(&h));
    assert!(!wire::routed("play.example.org"));
    assert_eq!(wire::target(&h), Some(("play.example.org:25570", "1.8.x")));
}

// The callbacks are process-wide: one test sets them all, in order.
#[test]
fn routing_redirects_and_signing_in() {
    // Routing happens only on the threads that join and ping.
    wire::set_router(|address, using| {
        Some(wire::internal_host(
            address,
            address,
            if using == wire::Use::Join { "1.8.x" } else { "auto" },
        ))
    });
    assert_eq!(wire::route("example.org"), None, "the test thread routes nothing");
    let joined = std::thread::Builder::new()
        .name("horizon-net".into())
        .spawn(|| wire::route("example.org"))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(joined.as_deref(), Some("example.org\u{7}example.org\u{7}1.8.x"));
    let pinged = std::thread::Builder::new()
        .name("horizon-ping".into())
        .spawn(|| wire::route("example.org"))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(pinged.as_deref(), Some("example.org\u{7}example.org\u{7}auto"));
    // An address already routed isn't routed again.
    assert_eq!(wire::route_for(&joined.unwrap(), wire::Use::Join), None);

    // A routed host's SRV lookup points at ViaProxy, or says why it can't.
    assert_eq!(wire::redirect("_minecraft._tcp.example.org."), None);
    wire::set_proxy(|_| Ok(41234));
    assert_eq!(
        wire::redirect("_minecraft._tcp.a\u{7}a\u{7}1.8.x."),
        Some(("127.0.0.1".to_owned(), 41234))
    );
    wire::set_proxy(|_| Err("Java not found".into()));
    let (host, _) = wire::redirect("_minecraft._tcp.a\u{7}a\u{7}1.8.x.").unwrap();
    assert_eq!(host, "ViaHorizon: Java not found");

    // OpenAuthMod: the request is answered after joining the session.
    let hashes = Arc::new(Mutex::new(Vec::new()));
    let seen = hashes.clone();
    wire::set_join(move |hash| {
        seen.lock().unwrap().push(hash.to_owned());
        Ok(())
    });
    let request = wire::join_request(7, "-1f2e3d");
    let answer = wire::oam(wire::CUSTOM_QUERY, &request).unwrap().unwrap();
    assert_eq!(answer, vec![wire::CUSTOM_QUERY_ANSWER as u8, 7, 1, 1]);
    assert_eq!(*hashes.lock().unwrap(), vec!["-1f2e3d".to_owned()]);
    // Other packets and other channels pass.
    assert!(wire::oam(wire::CUSTOM_QUERY - 1, &request).is_none());
    let mut other = Vec::new();
    wire::varint(&mut other, 3);
    wire::string(&mut other, "velocity:player_info");
    assert!(wire::oam(wire::CUSTOM_QUERY, &other).is_none());
    assert!(
        wire::oam(wire::CUSTOM_QUERY, &[0xff]).is_none(),
        "a broken packet is left to the game"
    );
    // A failed join ends the login with its reason.
    wire::set_join(|_| Err("Invalid session".into()));
    assert_eq!(
        wire::oam(wire::CUSTOM_QUERY, &request),
        Some(Err("Invalid session".into()))
    );
    // Large transaction ids round trip as varints.
    let big = wire::answer(300, false);
    assert_eq!(big, vec![wire::CUSTOM_QUERY_ANSWER as u8, 0xac, 0x02, 1, 0]);
}

#[test]
fn which_joins_go_through_viaproxy() {
    let routed = via::route_with("mc.example.net", "1.8.x", Some(41234)).unwrap();
    assert_eq!(routed, "mc.example.net\u{7}mc.example.net\u{7}1.8.x");
    let with_port = via::route_with(" mc.example.net:25570 ", "auto", None).unwrap();
    assert_eq!(with_port, "mc.example.net\u{7}mc.example.net:25570\u{7}auto");
    // Joined as they are: Horizon's own version, friends' worlds, the
    // integrated server, IPv6 literals, ViaProxy itself.
    assert_eq!(via::route_with("mc.example.net", via::NATIVE, None), None);
    assert_eq!(via::route_with("friend:K7QM-3XPD", "1.8.x", None), None);
    assert_eq!(via::route_with("local", "1.8.x", None), None);
    assert_eq!(via::route_with("[::1]:25565", "1.8.x", None), None);
    assert_eq!(via::route_with("127.0.0.1:41234", "1.8.x", Some(41234)), None);
    assert_eq!(via::route_with("", "1.8.x", None), None);
    // A host too long for the handshake goes straight.
    let long = format!("{}.example", "a".repeat(200));
    assert_eq!(via::route_with(&long, "1.8.x", None), None);
}

#[test]
fn versions_and_settings() {
    assert_eq!(via::VERSIONS[0], via::AUTO);
    assert!(via::VERSIONS.contains(&"1.8.x") && via::VERSIONS.contains(&"26.3"));
    assert_eq!(via::find_version("1.8.X"), Some("1.8.x"));
    assert_eq!(via::find_version("Auto detect"), Some(via::AUTO));
    assert_eq!(via::find_version("1.99"), None);
    assert_eq!(via::label(via::AUTO), "Auto detect");
    assert_eq!(via::label("1.12.2"), "1.12.2");
    let s = via::parse_settings("enabled=false\nversion=1.12.2\n");
    assert!(!s.enabled);
    assert_eq!(s.version, "1.12.2");
    // Unknown or missing values keep the defaults: on, auto detect.
    let s = via::parse_settings("version=9.9\n");
    assert!(s.enabled);
    assert_eq!(s.version, via::AUTO);
}

#[test]
fn java_versions_are_read() {
    let t = "openjdk version \"21.0.12.1\" 2026-08-18\nOpenJDK Runtime Environment";
    assert_eq!(via::parse_java_version(t), Some(21));
    assert_eq!(via::parse_java_version("java version \"1.8.0_401\""), Some(8));
    assert_eq!(
        via::parse_java_version("Picked up JAVA_TOOL_OPTIONS: -X\nopenjdk version \"17\" 2021-09-14"),
        Some(17)
    );
    assert_eq!(via::parse_java_version("nothing here"), None);
}

#[test]
fn viaproxy_listens_on_the_loopback_and_reads_each_join() {
    let args = via::viaproxy_args(std::path::Path::new("/x/ViaProxy.jar"), 41234);
    let line = args.join(" ");
    assert!(line.contains("-jar /x/ViaProxy.jar cli"), "{line}");
    assert!(line.contains("--bind-address 127.0.0.1:41234"));
    assert!(line.contains("--wildcard-domain-handling INTERNAL"));
    assert!(line.contains("--auth-method OPENAUTHMOD"));
    assert_eq!(via::strip_ansi("\u{1b}[34m[00:45:04]\u{1b}[m ok"), "[00:45:04] ok");
}

/// A tar entry (ustar, as GNU tar writes the Java runtime).
fn entry(out: &mut Vec<u8>, name: &str, kind: u8, data: &[u8]) {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..107].copy_from_slice(b"0000755");
    h[124..135].copy_from_slice(format!("{:011o}", data.len()).as_bytes());
    h[156] = kind;
    h[257..265].copy_from_slice(b"ustar  \0");
    out.extend_from_slice(&h);
    out.extend_from_slice(data);
    out.resize(out.len().div_ceil(512) * 512, 0);
}

#[test]
fn archives_unpack_safely() {
    let dir = std::env::temp_dir().join(format!("hstudio-untar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut tar = Vec::new();
    entry(&mut tar, "jdk-21-jre/", b'5', b"");
    entry(&mut tar, "jdk-21-jre/bin/java", b'0', b"#!java");
    let long = format!("jdk-21-jre/lib/{}", "x".repeat(120));
    entry(&mut tar, "././@LongLink", b'L', format!("{long}\0").as_bytes());
    entry(&mut tar, "ignored", b'0', b"long");
    entry(&mut tar, "jdk-21-jre/legal/LICENSE", b'2', b"");
    tar.extend_from_slice(&[0u8; 1024]);
    via::untar(&tar[..], &dir).unwrap();
    assert_eq!(std::fs::read(dir.join("jdk-21-jre/bin/java")).unwrap(), b"#!java");
    assert_eq!(std::fs::read(dir.join(&long)).unwrap(), b"long");
    assert!(!dir.join("jdk-21-jre/legal/LICENSE").exists(), "links are left out");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("jdk-21-jre/bin/java"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "executables stay executable");
    }
    // Paths that climb out are refused.
    let mut evil = Vec::new();
    entry(&mut evil, "../evil", b'0', b"x");
    assert!(via::untar(&evil[..], &dir).is_err());
    assert!(!dir.parent().unwrap().join("evil").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scripts_see_and_change_viahorizon() {
    let mut game = FakeGame::new("via-api");
    let mut st = studio(&mut game);
    assert_eq!(
        console(
            &mut st,
            &mut game,
            "local s = via.status() return s.enabled, s.version, s.label, s.ready"
        ),
        vec!["true, \"auto\", \"Auto detect\", true"]
    );
    assert_eq!(console(&mut st, &mut game, "#via.versions()"), vec!["5"]);
    assert_eq!(console(&mut st, &mut game, "via.versions()[5].id"), vec!["\"1.8.x\""]);
    console(&mut st, &mut game, "via.version('1.8.x') via.enable(false)");
    assert_eq!(game.via.version, "1.8.x");
    assert!(!game.via.enabled);
    let out = console(&mut st, &mut game, "via.version('9.9')");
    assert!(out[0].contains("no version 9.9"), "{out:?}");
    assert_eq!(
        console(&mut st, &mut game, "via.log()[1]"),
        vec!["\"[ViaProxy] Binding proxy server to 127.0.0.1:41234\""]
    );
}

#[test]
fn the_viahorizon_mod_comes_installed_and_picks_versions() {
    let mut game = FakeGame::new("via-mod");
    let mut st = studio(&mut game);
    assert!(game.mods_dir().join("viahorizon.lua").is_file());
    assert_eq!(
        st.s.mods[mod_index(&st, "viahorizon")].state,
        ModState::Running,
        "{:?}",
        log(&st)
    );
    // /via 1.12.2 chooses without the screen.
    assert!(st.command("via 1.12.2"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.via.version, "1.12.2");
    assert!(
        game.chats.iter().any(|c| c.contains("joins as 1.12.2")),
        "{:?}",
        game.chats
    );
    // /via opens the screen; a click on a version chooses it.
    assert!(st.command("via"));
    st.frame(&mut game, 0.016);
    assert!(st.accept_screen());
    for _ in 0..8 {
        st.frame(&mut game, 0.05);
        game.drawn.clear();
        draw_screen(&mut st, &mut game);
    }
    let at = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text 1.8.x @"))
        .cloned()
        .unwrap_or_else(|| panic!("the list: {:?}", game.drawn));
    let xy: Vec<f32> = at
        .split('@')
        .nth(1)
        .unwrap()
        .trim_matches(|c| c == '[' || c == ' ')
        .split(']')
        .next()
        .unwrap()
        .split(", ")
        .map(|v| v.parse().unwrap())
        .collect();
    handle_screen(&mut st, &mut game, Input::Mouse(xy[0] + 2.0, xy[1] + 4.0));
    handle_screen(&mut st, &mut game, Input::Button { left: true, down: true });
    handle_screen(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    st.frame(&mut game, 0.05);
    draw_screen(&mut st, &mut game);
    assert_eq!(game.via.version, "1.8.x", "{:?}", log(&st));
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(
        st.s.mods[mod_index(&st, "viahorizon")].state,
        ModState::Running,
        "{:?}",
        log(&st)
    );
}

#[test]
fn pings_are_answered_on_the_network_thread_once_frames_stop() {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    let sent = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let ping = [0, 0, 0, 7];
    let log = sent.clone();
    wire::set_pong(move |body| {
        log.lock().unwrap().push(body.to_vec());
        true
    });
    // While frames come, the game answers its pings.
    wire::frame_at(Instant::now());
    assert!(!wire::answer_ping(62, &ping, 62, 45));
    // No frame for 2 seconds: the ping is answered here and dropped.
    wire::frame_at(Instant::now() - Duration::from_secs(2));
    assert!(wire::answer_ping(62, &ping, 62, 45));
    assert_eq!(sent.lock().unwrap().as_slice(), &[vec![45, 0, 0, 0, 7]]);
    // Other packets go on to the game, and so do pings with no connection
    // to answer on, or whose answer could not be sent.
    assert!(!wire::answer_ping(61, &ping, 62, 45));
    assert!(!wire::answer_ping(62, &[7], 62, 45));
    wire::clear_pong();
    assert!(!wire::answer_ping(62, &ping, 62, 45));
    wire::set_pong(|_| false);
    assert!(!wire::answer_ping(62, &ping, 62, 45));
    wire::clear_pong();
    assert_eq!(sent.lock().unwrap().len(), 1);
    // The hook's own reads pass through it.
    assert!(!wire::reading());
    {
        let _reading = wire::Reading::start();
        assert!(wire::reading());
    }
    assert!(!wire::reading());
}
