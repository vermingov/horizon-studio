fn horizon_studio_natrium_merge(
    items: &mut [ItemEntity],
    removed: &mut [bool],
    cells: &mut ItemCells,
    i: usize,
    max_stack: &dyn Fn(&ItemStack) -> u32,
) -> core::ops::ControlFlow<()> {
    // Natrium: the same merges, each stack's size looked up once.
    if horizon_core::natrium::ticks() {
        natrium_items::merge_with_neighbours(items, removed, cells, i, max_stack);
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
