fn horizon_studio_natrium_fog(value: Option<(f64, Vec3)>) -> Option<(f64, Vec3)> {
    // Natrium: a section wholly inside the fog isn't drawn.
    if let (Some((_, min)), Some(end)) = (value, horizon_core::natrium::fog_cut())
        && horizon_core::natrium::past(min.to_array(), end)
    {
        horizon_core::natrium::skipped_sections(1);
        return None;
    }
    value
}
