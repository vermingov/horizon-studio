fn horizon_studio_natrium_fog(value: bool, min: Vec3, max: Vec3) -> bool {
    // Natrium: a section (or a whole chunk column) wholly inside the fog
    // isn't drawn.
    if value
        && let Some(end) = horizon_core::natrium::fog_cut()
        && horizon_core::natrium::past_box(min.to_array(), max.to_array(), end)
    {
        horizon_core::natrium::skipped_sections(horizon_core::natrium::sections_in(
            min.to_array(),
            max.to_array(),
        ));
        return false;
    }
    value
}
