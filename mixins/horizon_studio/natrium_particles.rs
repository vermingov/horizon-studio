// Natrium's particle culling (in the game's `particles`, as
// `particles::natrium_particles`): `ParticleEngine::render` as Horizon has it
// (copied from the pinned revision), but a particle outside the view frustum
// is left out before its sprite, its light and its vertices are worked out.
// The frustum is the frame's camera frustum, kept as the renderer makes it.

use super::*;
use std::cell::Cell;

thread_local! {
    static VIEW: Cell<Option<crate::gfx::Frustum>> = const { Cell::new(None) };
}

/// The frame's camera frustum (camera-relative), from `camera_frustum`.
pub(crate) fn set_view(frustum: crate::gfx::Frustum) {
    VIEW.with(|v| v.set(Some(frustum)));
}

/// `ParticleEngine::render` with frustum culling; `false` when there is no
/// frustum yet (the game's own runs then).
pub(crate) fn render(
    engine: &ParticleEngine,
    camera: DVec3,
    yaw: f32,
    pitch: f32,
    partial: f32,
    light_at: &dyn Fn([i32; 3]) -> u8,
    out: &mut Vec<Vertex>,
) -> bool {
    let Some(view) = VIEW.with(Cell::get) else {
        return false;
    };
    let mut skipped = 0u64;
    // Camera.setRotation: rotationYXZ(PI - yRot, -xRot, 0).
    let base =
        Quat::from_rotation_y(std::f32::consts::PI - yaw.to_radians()) * Quat::from_rotation_x(-pitch.to_radians());
    for p in &engine.particles {
        if p.no_render {
            continue;
        }
        // Natrium: a particle outside the view costs nothing more.
        let a = f64::from(partial);
        let pos = DVec3::new(
            p.previous[0] + (p.position[0] - p.previous[0]) * a,
            p.previous[1] + (p.position[1] - p.previous[1]) * a,
            p.previous[2] + (p.position[2] - p.previous[2]) * a,
        ) - camera;
        let center = pos.as_vec3();
        let reach = FVec3::splat(p.quad_size(partial).abs() * 1.5 + 0.05);
        if !view.intersects(center - reach, center + reach) {
            skipped += 1;
            continue;
        }
        // SparkParticle.extract: twinkling sparks blink after a third of their life.
        if let Behaviour::Spark { twinkle: true, .. } = p.behaviour
            && p.age >= p.lifetime / 3
            && ((p.age + p.lifetime) / 3) % 2 != 0
        {
            continue;
        }
        // `SubmitNodeCollection.submitQuadParticleGroup`: the translucent
        // layer draws after translucent terrain, so a crumb of stained
        // glass cannot hide the glass behind it (`getLayer`).
        let (rect, layer) = match &p.sprite {
            Sprite::Index(i) => (
                engine.rects.get(*i as usize).copied().flatten(),
                p.ty.translucent_layer(),
            ),
            Sprite::Named(name) => match engine.atlas.get(name) {
                Some(&(rect, translucent)) => (Some(rect), translucent),
                None => (None, false),
            },
        };
        let Some(r) = rect else { continue };
        let rotation = if p.roll != 0.0 {
            base * Quat::from_rotation_z(p.o_roll + (p.roll - p.o_roll) * partial)
        } else {
            base
        };
        let size = p.quad_size(partial);
        let mut alpha = p.rgba[3];
        // OverlayParticle.extract: the flash fades as it pulses.
        if p.ty == ParticleType::Flash {
            alpha = 0.6 - (p.age as f32 + partial - 1.0) * 0.25 * 0.5;
        }
        if let Behaviour::FlyTowards { alpha: la, .. } = p.behaviour {
            alpha = lifetime_alpha(la, p.age, p.lifetime, partial);
        }
        let mut rotations = [Some(rotation), None];
        let mut rgb = [p.rgba[0], p.rgba[1], p.rgba[2]];
        match p.behaviour {
            // ShriekParticle.extract: two tilted quads, fading out.
            Behaviour::Shriek { delay } => {
                if delay > 0 {
                    continue;
                }
                alpha = 1.0 - ((p.age as f32 + partial) / p.lifetime as f32).clamp(0.0, 1.0);
                rotations = [
                    Some(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_3)),
                    Some(Quat::from_euler(
                        glam::EulerRot::YXZ,
                        -std::f32::consts::PI,
                        std::f32::consts::FRAC_PI_3,
                        0.0,
                    )),
                ];
            }
            // VibrationSignalParticle.extract: two quads along the flight.
            Behaviour::Vibration {
                rot,
                rot_o,
                pitch,
                pitch_o,
                ..
            } => {
                let sway = ((p.age as f32 + partial - std::f32::consts::TAU) * 0.05).sin() * 2.0;
                let lerped_rot = rot_o + (rot - rot_o) * partial;
                let lerped_pitch = pitch_o + (pitch - pitch_o) * partial + std::f32::consts::FRAC_PI_2;
                rotations = [
                    Some(
                        Quat::from_rotation_y(lerped_rot)
                            * Quat::from_rotation_x(-lerped_pitch)
                            * Quat::from_rotation_y(sway),
                    ),
                    Some(
                        Quat::from_rotation_y(-std::f32::consts::PI + lerped_rot)
                            * Quat::from_rotation_x(lerped_pitch)
                            * Quat::from_rotation_y(sway),
                    ),
                ];
            }
            // DustColorTransitionParticle.lerpColors.
            Behaviour::Transition { from, to } => {
                let t = (p.age as f32 + partial) / (p.lifetime as f32 + 1.0);
                rgb = std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t);
            }
            _ => {}
        }
        let block = ParticleLevel::block_at(p.position);
        let packed = light_at(block);
        let (sky_light, block_light) =
            p.light_coords(partial, u32::from(packed >> 4) * 16, u32::from(packed & 15) * 16);
        let light = sky_light | block_light << 8;
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u32;
        let color = c(alpha) << 24 | c(rgb[2]) << 16 | c(rgb[1]) << 8 | c(rgb[0]);
        let (du, dv) = (r.u1 - r.u0, r.v1 - r.v0);
        let (u0, v0, u1, v1) = (
            r.u0 + du * p.uv[0],
            r.v0 + dv * p.uv[1],
            r.u0 + du * p.uv[2],
            r.v0 + dv * p.uv[3],
        );
        for rotation in rotations.into_iter().flatten() {
            for (n, uv) in [
                ((1.0, -1.0), [u1, v1]),
                ((1.0, 1.0), [u1, v0]),
                ((-1.0, 1.0), [u0, v0]),
                ((-1.0, -1.0), [u0, v1]),
            ] {
                let v = center + rotation * FVec3::new(n.0, n.1, 0.0) * size;
                out.push(Vertex {
                    normal: [0; 4],
                    pos: v.to_array(),
                    uv,
                    layer: if layer {
                        crate::gfx::TRANSLUCENT_PARTICLE_OVERLAY
                    } else {
                        0
                    } | u32::from(super::hdr_tier(p.ty)) << 16,
                    color,
                    light,

                    glint_uv: [0.0, 0.0],
                });
            }
        }
    }
    horizon_core::natrium::skipped_particles(skipped);
    true
}
