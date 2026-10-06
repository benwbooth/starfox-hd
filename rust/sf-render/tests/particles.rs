//! Completed particle commands share the shipping mesh painter order.
use sf_core::particles::{
    ParticleDraw, ParticleFrame, ParticleOwner, ParticlePrimitive, ParticleWork,
    PARTICLE_OBJECT_FLAG,
};
use sf_render::{
    draw_list::{DrawListEntry, ShadowStyle, DL_FLAG_SCALED_SPRITE, DL_FLAG_TEXT, DL_FLAG_VISIBLE},
    renderer::{config_from_repo_root, FrameInputs, Renderer},
    shapes::{decode_shape_palette, game_palette_bgr},
};

const WIDTH: usize = 256;
const HEIGHT: usize = 224;

fn owner_entry(depth: i32) -> DrawListEntry {
    DrawListEntry {
        obj_id: 2,
        shape_id: u16::MAX,
        z: depth << 16,
        // Particle dispatch must precede shape lookup, text and scaled sprites.
        flags: DL_FLAG_VISIBLE | DL_FLAG_TEXT | DL_FLAG_SCALED_SPRITE,
        sflags: PARTICLE_OBJECT_FLAG,
        ..Default::default()
    }
}

fn frame_at(x: u8, color: u8) -> ParticleFrame {
    ParticleFrame {
        draws: vec![ParticleDraw {
            owner: ParticleOwner::new(2).unwrap(),
            particles_visited: 3,
            primitives: vec![
                ParticlePrimitive::Dot {
                    corner: [x, 96],
                    palette_index: color,
                },
                ParticlePrimitive::Dot {
                    corner: [x, 96],
                    palette_index: 0,
                },
                ParticlePrimitive::Line {
                    start: [x, 100],
                    end: [x + 5, 105],
                    palette_index: color,
                },
            ],
        }],
        work: ParticleWork {
            owner_slots_scanned: 300,
            particles_visited: 3,
            aging_slots_scanned: 300,
            ..Default::default()
        },
    }
}

fn render(
    renderer: &mut Renderer,
    entries: &[DrawListEntry],
    inputs: &FrameInputs<'_>,
    alpha: f32,
) -> Vec<u8> {
    renderer.begin_frame();
    renderer.submit(entries, entries, alpha, inputs);
    renderer.end_frame();
    renderer.read_pixels_rgb()
}

#[test]
fn particle_pixels_keep_completed_state_and_obey_mesh_occlusion_in_both_render_modes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = config_from_repo_root(&root);
    config.shadow_style = ShadowStyle::Disabled;
    let mut renderer = Renderer::new_headless(WIDTH as i32, HEIGHT as i32, &config)
        .expect("GPU required for source/HD particle integration");
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let previous = frame_at(61, 6);
    let current = frame_at(137, 12);
    let empty = ParticleFrame::default();
    for source_resolution in [true, false] {
        for (prior, next, alpha) in [
            (None, &previous, 0.0),
            (Some(&previous), &current, 0.0),
            (Some(&previous), &current, 0.5),
            (Some(&previous), &current, 1.0),
            (Some(&current), &empty, 0.5),
            (Some(&current), &empty, 1.0),
        ] {
            let expected = if alpha < 1.0 {
                prior.unwrap_or(next)
            } else {
                next
            };
            let inputs = FrameInputs {
                source_resolution,
                particle_frame: Some(next),
                previous_particle_frame: prior,
                ..Default::default()
            };
            let actual = render(&mut renderer, &[owner_entry(1000)], &inputs, alpha);
            let palette = decode_shape_palette(game_palette_bgr(inputs.scene_style.game_palette));
            let mut indices = vec![0; WIDTH * HEIGHT];
            let mut rgb = vec![0; WIDTH * HEIGHT * 3];
            for draw in &expected.draws {
                for pixel in draw.pixels() {
                    let position = (usize::from(pixel.y) + 16) * WIDTH + usize::from(pixel.x) + 16;
                    indices[position] = pixel.palette_index;
                    let color = palette[usize::from(pixel.palette_index)]
                        .map(|c| (c * 255.0).round() as u8);
                    rgb[position * 3..position * 3 + 3].copy_from_slice(&color);
                }
            }
            assert_eq!(actual, rgb, "source={source_resolution} alpha={alpha}");
            if source_resolution {
                assert_eq!(renderer.source_bitmap_indices(), indices);
                assert_eq!(renderer.source_frame_workload().particles, expected.work);
                assert_eq!(
                    renderer.source_frame_workload().point_samples,
                    if expected.draws.is_empty() { 0 } else { 8 }
                );
            }
        }

        let inputs = FrameInputs {
            source_resolution,
            ..Default::default()
        };
        let mesh = DrawListEntry {
            obj_id: 1,
            shape_id: 2,
            z: 700 << 16,
            flags: DL_FLAG_VISIBLE,
            ..Default::default()
        };
        let mesh_pixels = render(&mut renderer, &[mesh], &inputs, 1.0);
        let mut covering_particles = frame_at(112, 6);
        covering_particles.draws[0].primitives = (80..112)
            .step_by(2)
            .flat_map(|y| {
                (96..128).step_by(2).map(move |x| ParticlePrimitive::Dot {
                    corner: [x, y],
                    palette_index: 6,
                })
            })
            .collect();
        let inputs = FrameInputs {
            particle_frame: Some(&covering_particles),
            ..inputs
        };
        let particle_pixels = render(&mut renderer, &[owner_entry(1000)], &inputs, 1.0);
        let overlaps = mesh_pixels
            .chunks_exact(3)
            .zip(particle_pixels.chunks_exact(3))
            .filter(|(mesh, particle)| **mesh != [0; 3] && **particle != [0; 3])
            .count();
        assert!(
            overlaps > 20,
            "test must cover visible geometry: {overlaps}"
        );
        for depth in [300, 1100] {
            for reversed in [false, true] {
                let mut entries = [mesh, owner_entry(depth)];
                if reversed {
                    entries.reverse();
                }
                let actual = render(&mut renderer, &entries, &inputs, 1.0);
                for (position, ((actual, mesh), particle)) in actual
                    .chunks_exact(3)
                    .zip(mesh_pixels.chunks_exact(3))
                    .zip(particle_pixels.chunks_exact(3))
                    .enumerate()
                {
                    let expected = if depth == 300 {
                        if particle != [0; 3] {
                            particle
                        } else {
                            mesh
                        }
                    } else if mesh != [0; 3] {
                        mesh
                    } else {
                        particle
                    };
                    assert_eq!(
                        actual,
                        expected,
                        "source={source_resolution} depth={depth} reversed={reversed} xy={:?}",
                        (position % WIDTH, position / WIDTH)
                    );
                }
            }
        }
    }
    renderer.shutdown();
}
