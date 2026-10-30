fn horizon_studio_natrium_particles(
    engine: &ParticleEngine,
    camera: DVec3,
    yaw: f32,
    pitch: f32,
    partial: f32,
    light_at: &dyn Fn([i32; 3]) -> u8,
    out: &mut Vec<Vertex>,
) -> core::ops::ControlFlow<()> {
    // Natrium: particles outside the view cost nothing.
    if horizon_core::natrium::particles()
        && natrium_particles::render(engine, camera, yaw, pitch, partial, light_at, out)
    {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
