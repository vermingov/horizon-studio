fn horizon_studio_natrium_fog_inside(value: bool, min: Vec3, max: Vec3) -> bool {
    // Natrium: a chunk column entirely in view whose far side reaches into the
    // fog has its sections tested one by one (natrium-fog cuts those past it).
    if value
        && let Some(end) = horizon_core::natrium::fog_cut()
        && horizon_core::natrium::reaches(min.to_array(), max.to_array(), end)
    {
        return false;
    }
    value
}
