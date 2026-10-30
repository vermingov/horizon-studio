// ViaHorizon's sign-in (in the game's `net`): when ViaProxy asks the client to
// join an online-mode server's session (OpenAuthMod), the join uses the
// account the player joins as, through the game's own session service.

use super::{AuthSession, Language, NOT_SIGNED_IN, tr};
use std::sync::Arc;

pub(crate) fn sign_in(account: Option<Arc<AuthSession>>, lang: Arc<Language>) {
    horizon_protocol::hstudio_via_net::set_join(move |hash| match &account {
        Some(a) => crate::auth::join_server(a, hash).map_err(|e| {
            let reason = match e.reason() {
                Ok(key) => tr(&lang, key),
                Err(message) => message.to_owned(),
            };
            format!("ViaHorizon couldn't sign you in to the server: {reason}")
        }),
        None => Err(NOT_SIGNED_IN.to_owned()),
    });
}
