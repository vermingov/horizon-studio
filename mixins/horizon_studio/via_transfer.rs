fn horizon_studio_via_transfer(auth: &Option<Arc<AuthSession>>, lang: &Arc<Language>) {
    // The account a join is made as signs in for ViaProxy.
    hstudio_via_auth::sign_in(auth.clone(), lang.clone());
}
