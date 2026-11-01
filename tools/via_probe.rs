// ViaHorizon's end-to-end probe, built inside the modded game's workspace by
// tools/via_e2e.py (as crates/horizon-client/examples/hstudio_via_probe.rs).
// It runs the real manager and the game's own network code: joins go through
// net::Session::connect and the server list's ping through net::ping, as the
// game makes them, so the hooks route them through ViaProxy.
//
//     hstudio_via_probe <data dir> <version> <offline address> [<online address>]
//
// Prints one line per step (READY, PING, JOINED, SIGNED-IN, ...); exits 1 on a failure.

use horizon_client::net::{Event, Session, hstudio_via as via, ping};
use horizon_protocol::hstudio_via_net as wire;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The client information a join sends: view distance 8, as the game's own test.
fn info() -> horizon_protocol::packets::configuration::ClientInformation {
    horizon_protocol::packets::configuration::ClientInformation::new(8)
}

fn fail(what: String) -> ! {
    println!("FAILED {what}");
    for line in via::log().iter().rev().take(25).rev() {
        println!("  | {line}");
    }
    std::process::exit(1);
}

fn join(address: &str, name: &str) -> Result<String, String> {
    let lang = Arc::new(horizon_protocol::text::Language::default());
    let session = Session::connect(address, name, None, info(), lang, std::path::PathBuf::new());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(90) {
        for e in session.poll() {
            match e {
                Event::Joined { name, uuid, registries } => {
                    let _ = registries;
                    return Ok(format!("{name} {uuid:032x}"));
                }
                Event::Disconnected { reason, .. } => return Err(reason),
                Event::Status(s) => println!("STATUS {s}"),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("no join in 90 s".into())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (dir, version, offline) = (&args[1], &args[2], &args[3]);
    via::configure(std::path::PathBuf::from(dir));
    via::set_enabled(true);
    via::set_version(version).unwrap_or_else(|e| fail(e));
    let start = Instant::now();
    loop {
        match via::status() {
            via::Status::Ready(port) => {
                println!("READY {port} {}", via::status_text());
                break;
            }
            via::Status::Failed(why) => fail(format!("start: {why}")),
            _ if start.elapsed() > Duration::from_secs(300) => fail(format!("start: {}", via::status_text())),
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }

    // The server list's ping, from a ping thread as the game makes it.
    let address = offline.clone();
    let pinged = std::thread::Builder::new()
        .name("horizon-ping".into())
        .spawn(move || ping(&address, Duration::from_secs(10)))
        .unwrap()
        .join()
        .unwrap();
    match pinged {
        Ok((json, ms)) => println!("PING {ms} ms {}", json["version"]),
        Err(e) => fail(format!("ping: {e}")),
    }

    match join(offline, "Tester") {
        Ok(who) => println!("JOINED offline {who}"),
        Err(e) => fail(format!("offline join: {e}")),
    }

    if let Some(online) = args.get(4) {
        // No account here: the hook's sign-in would refuse. The probe signs
        // in instead, as the game would with the player's session.
        let hashes = Arc::new(Mutex::new(Vec::<String>::new()));
        let lang = Arc::new(horizon_protocol::text::Language::default());
        let session = Session::connect(online, "Tester", None, info(), lang, std::path::PathBuf::new());
        let seen = hashes.clone();
        wire::set_join(move |hash| {
            seen.lock().unwrap().push(hash.to_owned());
            Ok(())
        });
        let start = Instant::now();
        let mut joined = false;
        while !joined && start.elapsed() < Duration::from_secs(90) {
            for e in session.poll() {
                match e {
                    Event::Joined { name, .. } => {
                        println!("JOINED online {name}");
                        joined = true;
                    }
                    Event::Disconnected { reason, .. } => fail(format!("online join: {reason}")),
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !joined {
            fail("online join: no join in 90 s".into());
        }
        println!("SIGNED-IN {:?}", hashes.lock().unwrap());
        drop(session);
        // Not signed in: the join ends with Horizon's reason.
        let refused = Session::connect(online, "Tester", None, info(), Arc::new(Default::default()), std::path::PathBuf::new());
        let start = Instant::now();
        'wait: while start.elapsed() < Duration::from_secs(60) {
            for e in refused.poll() {
                if let Event::Disconnected { reason, .. } = e {
                    println!("REFUSED {reason}");
                    break 'wait;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    via::stop();
    println!("DONE");
}
