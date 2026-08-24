//! OSS-010 — Criterion benchmarks for the systems that run every tick.
//!
//! # Why this file exists
//!
//! `BENCHMARK_BASELINE.md` declares its own numbers to be proxies, because running the full backend
//! crashed the dev machine. That left the project's "60 FPS real-time" claim as something no one had
//! ever measured, while decisions about LOD, brain memory budget and resident agent count were being
//! made on top of it (`STATE_OF_THE_PROJECT.md` §3.2).
//!
//! The constraint that produced the proxies is real and is not going away: **do not run
//! `npm run tauri dev` / `cargo run` on the dev machine.** So this file does not try. Every benchmark
//! here drives one system, or one pure function, on data it builds itself — no Tauri, no window, no
//! GPU device, no simulation thread. That is the whole reason Criterion fits: a 60 FPS frame budget
//! is 16.7 ms, and a frame budget is a *sum over systems*, so per-system numbers are what the claim
//! is actually made of.
//!
//! # What a number here does and does not prove
//!
//! It proves what one system costs, at a stated size, on the machine named in the report. It does
//! **not** prove the frame budget: the tick also pays for scheduling, change detection, the emit
//! thread and everything not benchmarked below. Read these as a lower bound on the frame, never as
//! the frame.
//!
//! # Sizes are the real sizes
//!
//! `MapSettings::default()` is 256², matching the shared World Artifact, and `ResourceField` is
//! built at that size. Benchmarking a 32² field would have produced a comfortable number for a
//! workload the engine never runs. Where a size is a choice rather than a constant, it is stated in
//! the benchmark id.
//!
//! Run with:  `cargo bench --bench tick_systems`
//! See:       `docs/how-to/BENCHMARKING.md`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::model::ActorCriticModel;
use anima_engine_lib::core::dynamic_fields::DynamicFields;
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::ecs::{MapBounds, Position, Prey, Velocity};
use anima_engine_lib::core::terrain::{BiomeType, MapSettings, TerrainMap};
use anima_engine_lib::core::training::{
    a2c_loss, ACTION_DIM, BATCH_SIZE, DISCOUNT, HIDDEN_DIM, STATE_DIM,
};
use anima_engine_lib::core::world_artifact::{
    map_biome_backend_to_frontend, WorldArtifact, CANONICAL_WORLD_SCALE,
};
use anima_engine_lib::physics::{
    integrate_physics_system, rebuild_spatial_grid_system, RigidBody, SpatialCollider,
    SpatialHashGrid,
};

use bevy_ecs::prelude::*;
use burn::backend::Autodiff;
use burn::tensor::{Data, Shape, Tensor};
use glam::Vec3;

/// The world size the engine actually runs at (`MapSettings::default()`), not a convenient one.
const WORLD_W: usize = 256;
const WORLD_H: usize = 256;

/// Agent counts to sweep. 100 is a debugging population; 1_000 is the scale at which the per-system
/// cost starts to matter against a 16.7 ms frame.
const AGENT_COUNTS: [usize; 3] = [100, 1_000, 10_000];

// ---- Fixtures ---------------------------------------------------------------------------------

/// A mixed-biome resource field at the real world size, half-depleted.
///
/// `from_biomes` starts every cell **at** its carrying capacity, and logistic growth at `r == r_max`
/// is exactly zero — so a fresh field would benchmark the early-exit path rather than the work. Half
/// capacity is what makes this measure regrowth.
fn resource_field() -> ResourceField {
    let biomes: Vec<u8> = (0..WORLD_W * WORLD_H)
        .map(|i| match i % 4 {
            0 => BiomeType::Rainforest as u8,
            1 => BiomeType::Grassland as u8,
            2 => BiomeType::TemperateForest as u8,
            _ => BiomeType::Desert as u8,
        })
        .collect();
    let mut f = ResourceField::from_biomes(
        &biomes, WORLD_W, WORLD_H, -100.0, -100.0, 100.0, 100.0, 0.02,
    );
    for cell in f.r.iter_mut() {
        *cell *= 0.5;
    }
    f
}

/// Terrain at the real size. Generation is expensive (erosion droplets scale with cell count), so
/// every benchmark that needs it builds it once outside the measured closure.
fn terrain() -> TerrainMap {
    TerrainMap::generate(&MapSettings {
        width: WORLD_W,
        height: WORLD_H,
        ..MapSettings::default()
    })
}

/// A world holding `n` moving bodies, half of them prey, all with a homeostatic state so the
/// depletion branch in `integrate_physics_system` is actually reached rather than short-circuited.
fn physics_world(n: usize) -> World {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));

    for i in 0..n {
        // Spread the bodies out and give them small distinct velocities. Nothing applies force, so
        // velocity stays bounded across a long benchmark run and positions drift linearly -- no risk
        // of the numbers walking into infinity and changing what is being measured.
        let f = i as f32;
        let mut e = world.spawn((
            Position(Vec3::new(f % 100.0, 0.0, (f / 100.0).floor())),
            Velocity(Vec3::new(0.01, 0.0, 0.01)),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::new(0.01, 0.0, 0.01),
                force: Vec3::ZERO,
            },
            HomeostaticState {
                energy: 50.0,
                energy_target: 100.0,
                hydration: 50.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        ));
        if i % 2 == 0 {
            e.insert(Prey);
        }
    }
    world
}

/// The same population, plus the collider and grid that `rebuild_spatial_grid_system` needs.
fn spatial_world(n: usize) -> World {
    let mut world = physics_world(n);
    let bounds = MapBounds {
        min: Vec3::new(-128.0, 0.0, -128.0),
        max: Vec3::new(128.0, 0.0, 128.0),
    };
    let entities: Vec<Entity> = world.iter_entities().map(|e| e.id()).collect();
    for id in entities {
        world.entity_mut(id).insert(SpatialCollider { radius: 0.5 });
    }
    world.insert_resource(SpatialHashGrid::new_prepopulated(10.0, &bounds));
    world.insert_resource(bounds);
    world
}

// ---- 1. Resource field regrowth ---------------------------------------------------------------

/// The claim under test is written into `ResourceField::REGROWTH_STRIDE`: the unstrided path cost
/// four full passes per tick, measured at ~4.2 ms, and striding turned that into two passes over a
/// quarter of the cells. Both variants are benchmarked so the ratio is a measurement rather than an
/// assertion in a doc comment.
fn bench_resource_field(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick/resource_field");
    group.throughput(Throughput::Elements((WORLD_W * WORLD_H) as u64));

    let dt = 1.0 / 60.0;

    group.bench_function("step_regrowth/256x256", |b| {
        let mut field = resource_field();
        b.iter(|| field.step_regrowth(black_box(dt), black_box(1.0)));
    });

    group.bench_function("step_regrowth_gated/256x256", |b| {
        let mut field = resource_field();
        b.iter(|| field.step_regrowth_gated(black_box(dt), black_box(1.0), black_box(1.0e9)));
    });

    let stride = ResourceField::REGROWTH_STRIDE;
    group.bench_function("step_regrowth_gated_strided/256x256", |b| {
        let mut field = resource_field();
        let mut phase = 0usize;
        b.iter(|| {
            // `dt * stride` is what the caller is contracted to pass, so each cell advances at the
            // same rate while being visited once every `stride` ticks.
            let out = field.step_regrowth_gated_strided(
                black_box(dt * stride as f32),
                black_box(1.0),
                black_box(1.0e9),
                phase,
                stride,
            );
            phase = (phase + 1) % stride;
            out
        });
    });

    group.finish();
}

// ---- 2. Dynamic fields ------------------------------------------------------------------------

fn bench_dynamic_fields(c: &mut Criterion) {
    let map = terrain();

    let mut group = c.benchmark_group("tick/dynamic_fields");
    group.throughput(Throughput::Elements((WORLD_W * WORLD_H) as u64));

    group.bench_function("step_water/256x256", |b| {
        let mut fields = DynamicFields::from_terrain(&map);
        b.iter(|| fields.step_water());
    });

    group.bench_function("step_erosion/256x256", |b| {
        let mut fields = DynamicFields::from_terrain(&map);
        b.iter(|| fields.step_erosion());
    });

    group.bench_function("step_soil/256x256", |b| {
        let mut fields = DynamicFields::from_terrain(&map);
        let mut tick = 0u64;
        b.iter(|| {
            fields.step_soil(tick, &[]);
            tick += 1;
        });
    });

    group.finish();
}

// ---- 3. Physics integration -------------------------------------------------------------------

fn bench_physics(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick/physics");

    for n in AGENT_COUNTS {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("integrate_physics_system", n),
            &n,
            |b, &n| {
                let mut world = physics_world(n);
                let mut schedule = Schedule::default();
                schedule.add_systems(integrate_physics_system);
                // The schedule is initialised once; `run` on an already-built schedule is what the
                // simulation loop does per tick, so that is what gets measured.
                schedule.run(&mut world);
                b.iter(|| schedule.run(&mut world));
            },
        );
    }

    group.finish();
}

// ---- 4. Spatial query -------------------------------------------------------------------------

fn bench_spatial(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial/grid");

    for n in AGENT_COUNTS {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("rebuild_spatial_grid_system", n),
            &n,
            |b, &n| {
                let mut world = spatial_world(n);
                let mut schedule = Schedule::default();
                schedule.add_systems(rebuild_spatial_grid_system);
                schedule.run(&mut world);
                b.iter(|| schedule.run(&mut world));
            },
        );
    }

    group.finish();
}

// ---- 5. Learner objective ---------------------------------------------------------------------

type Ad = Autodiff<burn_ndarray::NdArray<f32>>;

/// Flat weights for the real architecture, in the layout `from_flat_weights` reads.
fn flat_weights() -> Vec<f32> {
    let len = (STATE_DIM * HIDDEN_DIM + HIDDEN_DIM)
        + (HIDDEN_DIM * HIDDEN_DIM + HIDDEN_DIM)
        + (HIDDEN_DIM * ACTION_DIM + ACTION_DIM)
        + (HIDDEN_DIM + 1);
    // All-positive so the ReLU trunk passes signal through; a mostly-dead trunk would benchmark
    // the cheap path.
    (0..len).map(|k| 0.05 + 0.01 * ((k % 7) as f32)).collect()
}

fn tensor<const D: usize>(data: Vec<f32>, shape: [usize; D]) -> Tensor<Ad, D> {
    Tensor::<Ad, D>::from_data(Data::new(data, Shape::new(shape)), &Default::default())
}

/// `a2c_loss` is the single A2C objective ADR-0003 requires both learners to share, so its cost is
/// paid once per optimiser step on the learner thread — off the tick path, but on the same CPU.
/// Benchmarked at the exact `BATCH_SIZE` the learner accumulates before stepping.
fn bench_learner(c: &mut Criterion) {
    let weights = flat_weights();
    let model = ActorCriticModel::<Ad>::from_flat_weights(
        STATE_DIM,
        HIDDEN_DIM,
        ACTION_DIM,
        &weights,
        &Default::default(),
    )
    .expect("flat weights match the declared architecture");

    let mut group = c.benchmark_group("learner");
    group.throughput(Throughput::Elements(BATCH_SIZE as u64));

    group.bench_function("a2c_loss/batch32", |b| {
        b.iter(|| {
            // The tensors are rebuilt each iteration on purpose: the learner builds them fresh from
            // the transition buffer every step, so hoisting them out would measure a call the engine
            // never makes.
            let states = tensor(vec![0.1; BATCH_SIZE * STATE_DIM], [BATCH_SIZE, STATE_DIM]);
            let next_states = tensor(vec![0.2; BATCH_SIZE * STATE_DIM], [BATCH_SIZE, STATE_DIM]);
            let actions = tensor(vec![0.5; BATCH_SIZE * ACTION_DIM], [BATCH_SIZE, ACTION_DIM]);
            let rewards = tensor(vec![1.0; BATCH_SIZE], [BATCH_SIZE, 1]);
            a2c_loss(
                black_box(&model),
                states,
                next_states,
                actions,
                rewards,
                DISCOUNT,
            )
            .into_scalar()
        });
    });

    group.finish();
}

// ---- 6. World artifact encode/decode ----------------------------------------------------------

/// Build the artifact from a real terrain rather than from filler, so the byte length and the
/// biome histogram the encoder walks are the ones a shipped world produces. Biomes go through
/// `map_biome_backend_to_frontend` because that is the direction the artifact is written in; using
/// the raw backend ids would encode a world no frontend could read.
fn artifact_from(map: &TerrainMap) -> WorldArtifact {
    WorldArtifact {
        width: map.width,
        height: map.height,
        sea_level: 0.5,
        seed: 1337,
        generator_version: 20,
        world_scale: CANONICAL_WORLD_SCALE,
        elevation: map.elevations.clone(),
        moisture: map.moistures.clone(),
        temperature: map.temperatures.clone(),
        flow: map.flows.clone(),
        biome: map
            .biomes
            .iter()
            .map(|b| map_biome_backend_to_frontend(*b))
            .collect(),
    }
}

fn bench_artifact(c: &mut Criterion) {
    let map = terrain();
    let artifact = artifact_from(&map);
    let bytes = artifact.to_bytes();

    let mut group = c.benchmark_group("artifact");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    group.bench_function("to_bytes/256x256", |b| {
        b.iter(|| black_box(&artifact).to_bytes());
    });

    group.bench_function("from_bytes/256x256", |b| {
        b.iter(|| WorldArtifact::from_bytes(black_box(&bytes)).expect("round-trips"));
    });

    group.bench_function("checksum/256x256", |b| {
        b.iter(|| black_box(&artifact).checksum());
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_resource_field,
    bench_dynamic_fields,
    bench_physics,
    bench_spatial,
    bench_learner,
    bench_artifact,
);
criterion_main!(benches);
