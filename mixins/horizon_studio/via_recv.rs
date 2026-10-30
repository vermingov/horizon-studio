fn horizon_studio_via_recv(conn: &mut Connection) -> core::ops::ControlFlow<Result<(i32, Vec<u8>)>> {
    // OpenAuthMod's sign-in requests are answered here; every other packet
    // goes on as received.
    loop {
        let got = conn.reader.recv();
        let answer = match &got {
            Ok((id, payload)) => crate::hstudio_via_net::oam(*id, payload),
            Err(_) => None,
        };
        match answer {
            None => return core::ops::ControlFlow::Break(got),
            Some(Ok(body)) => {
                if let Err(e) = conn.writer.send(&body) {
                    return core::ops::ControlFlow::Break(Err(e));
                }
            }
            Some(Err(why)) => return core::ops::ControlFlow::Break(Err(Error::new(why))),
        }
    }
}
