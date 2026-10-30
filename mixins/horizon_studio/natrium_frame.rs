fn horizon_studio_natrium_frame(renderer: &Renderer, frame: &Frame<'_>) {
    // Natrium's fog culling applies only where what the fog hides is drawn
    // against the fog colour: no sky (the Nether, lava, powder snow, blindness,
    // darkness) or a sky as fogged as the terrain (water). Not on the title
    // screen, in the End, or under a shader pack (its shadows see further).
    let cut = frame.fog.as_ref().filter(|f| {
        !frame.title
            && !frame.end_sky
            && renderer.shader_pack_dimension().is_none()
            && (!frame.skybox || f.sky_end <= f.environmental_end)
            && f.environmental_end < f.render_distance_end
    });
    horizon_core::natrium::set_fog_cut(cut.map(|f| f.environmental_end));
}
