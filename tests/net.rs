// The network for scripts (hstudio_net): the address policy, HTTP and
// WebSockets, against servers on this machine (which scripts themselves may
// not reach; the tests allow it).

use horizon_studio::game::hstudio_net::{Event, Request, Socket, fetch, public, send};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

#[test]
fn only_the_internet_is_public() {
    for ip in [
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "192.168.1.1",
        "169.254.10.10",
        "100.64.0.1",
        "0.0.0.0",
        "255.255.255.255",
        "224.0.0.1",
        "::1",
        "fe80::1",
        "fd00::1",
        "::ffff:192.168.0.1",
    ] {
        assert!(!public(ip.parse::<IpAddr>().unwrap()), "{ip}");
    }
    for ip in ["1.1.1.1", "93.184.216.34", "2606:4700:4700::1111", "::ffff:8.8.8.8"] {
        assert!(public(ip.parse::<IpAddr>().unwrap()), "{ip}");
    }
}

/// An HTTP server answering one request with what it was sent.
fn http_echo() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        let mut len = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().unwrap();
            }
            if line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let mut body = vec![0; len];
        reader.read_exact(&mut body).unwrap();
        let first = head.lines().next().unwrap().to_owned();
        let reply = format!("{first}|{}", String::from_utf8_lossy(&body));
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 201 Created\r\nX-Echo: yes\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
            reply.len()
        )
        .unwrap();
    });
    port
}

#[test]
fn requests_reach_a_server_and_come_back() {
    let port = http_echo();
    let req = Request {
        url: format!("http://127.0.0.1:{port}/hello?x=1"),
        method: "post".into(),
        headers: vec![("Content-Type".into(), "text/plain".into())],
        body: b"ping".to_vec(),
    };
    let r = send(req, true).wait().unwrap();
    assert_eq!(r.status, 201);
    assert_eq!(r.status_message, "Created");
    assert!(r.success());
    assert!(
        r.headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("x-echo") && v == "yes")
    );
    assert_eq!(String::from_utf8_lossy(&r.body), "POST /hello?x=1 HTTP/1.1|ping");
    // Scripts can't reach this machine.
    let port = http_echo();
    let err = fetch(
        &Request {
            url: format!("http://127.0.0.1:{port}/"),
            ..Request::default()
        },
        false,
    )
    .unwrap_err();
    assert!(err.contains("can't reach"), "{err}");
    let _ = TcpStream::connect(("127.0.0.1", port));
    assert!(
        fetch(
            &Request {
                url: "file:///etc/passwd".into(),
                ..Request::default()
            },
            true
        )
        .is_err()
    );
}

/// A WebSocket server that answers the handshake, pings once, then echoes
/// until the client closes.
fn ws_echo() -> u16 {
    use sha1::{Digest, Sha1};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut key = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(v) = line.strip_prefix("Sec-WebSocket-Key:") {
                key = v.trim().to_owned();
            }
            if line == "\r\n" {
                break;
            }
        }
        let accept = b64(&Sha1::digest(
            format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
        ));
        write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        )
        .unwrap();
        // A ping first: the client answers with a pong.
        stream.write_all(&[0x89, 4, b'p', b'i', b'n', b'g']).unwrap();
        loop {
            let mut h = [0u8; 2];
            if reader.read_exact(&mut h).is_err() {
                return;
            }
            assert!(h[1] & 0x80 != 0, "clients mask their frames");
            let mut len = u64::from(h[1] & 0x7F);
            if len == 126 {
                let mut l = [0u8; 2];
                reader.read_exact(&mut l).unwrap();
                len = u64::from(u16::from_be_bytes(l));
            } else if len == 127 {
                let mut l = [0u8; 8];
                reader.read_exact(&mut l).unwrap();
                len = u64::from_be_bytes(l);
            }
            let mut mask = [0u8; 4];
            reader.read_exact(&mut mask).unwrap();
            let mut payload = vec![0u8; len as usize];
            reader.read_exact(&mut payload).unwrap();
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
            match h[0] & 0x0F {
                0xA => assert_eq!(payload, b"ping", "the pong carries the ping's data"),
                0x8 => {
                    let _ = stream.write_all(&[0x88, 2, 0x03, 0xE8]);
                    return;
                }
                op => {
                    // Echoed back, unmasked, in two fragments when long.
                    if payload.len() > 200 {
                        let (a, b) = payload.split_at(100);
                        frame(&mut stream, op, false, a);
                        frame(&mut stream, 0, true, b);
                    } else {
                        frame(&mut stream, op, true, &payload);
                    }
                }
            }
        }
    });
    port
}

fn frame(stream: &mut TcpStream, op: u8, fin: bool, payload: &[u8]) {
    let mut f = vec![if fin { 0x80 } else { 0 } | op];
    if payload.len() < 126 {
        f.push(payload.len() as u8);
    } else {
        f.push(126);
        f.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    f.extend_from_slice(payload);
    stream.write_all(&f).unwrap();
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() {
                A[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// The socket's events until `done` says enough (or five seconds pass).
fn until(sock: &Socket, done: impl Fn(&[Event]) -> bool) -> Vec<Event> {
    let start = Instant::now();
    let mut all = Vec::new();
    while !done(&all) && start.elapsed() < Duration::from_secs(5) {
        all.extend(sock.events());
        std::thread::sleep(Duration::from_millis(5));
    }
    all
}

#[test]
fn a_websocket_opens_echoes_and_closes() {
    let port = ws_echo();
    let sock = Socket::open(&format!("ws://127.0.0.1:{port}/chat"), true);
    let ev = until(&sock, |e| !e.is_empty());
    assert_eq!(ev, [Event::Open]);
    sock.send(b"hello", true);
    let long = vec![b'x'; 300];
    sock.send(&long, false);
    let ev = until(&sock, |e| e.len() >= 2);
    assert_eq!(
        ev,
        [Event::Message(b"hello".to_vec(), true), Event::Message(long, false)],
        "a fragmented message comes in whole"
    );
    sock.close();
    let ev = until(&sock, |e| e.iter().any(|x| matches!(x, Event::Closed(_))));
    assert_eq!(ev, [Event::Closed(None)], "closed when asked: no reason");
}

#[test]
fn a_websocket_to_this_machine_is_refused_for_scripts() {
    let port = ws_echo();
    let sock = Socket::open(&format!("ws://127.0.0.1:{port}/"), false);
    let ev = until(&sock, |e| !e.is_empty());
    match &ev[..] {
        [Event::Closed(Some(why))] => assert!(why.contains("can't reach"), "{why}"),
        other => panic!("{other:?}"),
    }
    let _ = TcpStream::connect(("127.0.0.1", port));
    let sock = Socket::open("http://example.com", true);
    let ev = until(&sock, |e| !e.is_empty());
    assert!(matches!(&ev[..], [Event::Closed(Some(_))]), "{ev:?}");
}

/// Over the internet, with the system's certificates (not run by default).
#[test]
#[ignore]
fn https_and_wss_on_the_internet() {
    let r = fetch(
        &Request {
            url: "https://raw.githubusercontent.com/vermingov/horizon-studio/main/README.md".into(),
            ..Request::default()
        },
        false,
    )
    .unwrap();
    assert_eq!(r.status, 200);
    assert!(String::from_utf8_lossy(&r.body).starts_with("# Horizon Studio"));
    let sock = Socket::open("wss://ws.postman-echo.com/raw", false);
    let ev = until(&sock, |e| !e.is_empty());
    assert_eq!(ev, [Event::Open], "{ev:?}");
    sock.send(b"hello over tls", true);
    let ev = until(&sock, |e| !e.is_empty());
    assert_eq!(ev, [Event::Message(b"hello over tls".to_vec(), true)]);
}
