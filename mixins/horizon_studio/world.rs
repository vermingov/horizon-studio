// As the world's entity geometry is made: the scripts' on("world") shapes.
fn horizon_studio_world(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    crate::game::hstudio_bridge::world(game, camera, out);
}
