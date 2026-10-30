fn horizon_studio_natrium_view(value: Frustum) -> Frustum {
    // Natrium: the frame's camera frustum, for particle culling.
    crate::particles::natrium_particles::set_view(value);
    value
}
