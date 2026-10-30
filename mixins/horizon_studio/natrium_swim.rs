fn horizon_studio_natrium_swim(
    finder: &mut Finder<'_, '_>,
    pos: u32,
    out: &mut Vec<u32>,
) -> core::ops::ControlFlow<()> {
    // Natrium: the same neighbours, without a hash map per node.
    if horizon_core::natrium::ticks() {
        natrium_paths::swim_neighbors(finder, pos, out);
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
