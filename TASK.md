# TASK — fix two blocking regressions in the current working tree

You are the implementer. The diagnosis below is already **measured and proven**; do not re-litigate
it, and do not go looking for other things to change. Implement exactly A, B and C.

Repo: `E:\Project\Anima-Engine`. Branch `feat/oss-071b-live-tracker`, HEAD `043930c`.
All work is uncommitted working-tree changes. **Do not `git commit`, `git stash`, `git checkout`,
`git reset`, or switch branches.** Other agents share this tree.

---

## HARD RULES (violating any of these makes the work worthless)

1. **Never move, relax, or delete a test fixture to make a test pass.** The previous pass did exactly
   that (it moved a raycast fixture from x=99.0 to x=90.0 to hide the bug in task A) and that is why
   this task exists. If a test fails, fix the *code*, or report that you cannot.
2. **Do not delete tests.** The one exception is named explicitly in task A.
3. **Do not touch anything not listed here.** Specifically leave alone:
   `evolution/genotype.rs` (`total_mass`), `ai/cpg.rs`, `physics/dynamics.rs`,
   `core/simulation_loop.rs`, `Terrain.tsx`'s removed `useFrame` wave animation, `tsconfig.json`,
   `src/types/generated/**`, `scripts/check_test_targets.mjs`, `NOTICE`, `AUDIT_REPORT.md`.
4. **Do not run `cargo test` or `npm run test:frontend`** — they take minutes and the orchestrator
   runs them afterwards. You may run `cargo check` and `npx tsc --noEmit -p tsconfig.json`.
5. Match surrounding code style. This codebase writes explanatory comments that say *why*; no
   `any`; no Prettier reformatting of untouched lines.

---

## A. Sensor raycast returns the agent's own collider — every agent is blind

### Proven behaviour

`evolution/genotype.rs:154` spawns the **root agent entity with its own `SpatialCollider`**.
`ai/model.rs:935` casts the sensor ray **from that same `Position`** — so the ray origin sits exactly
at the centre of the agent's own collider.

The current working tree changed `intersect_sphere` in `src-tauri/src/physics/spatial.rs` so that a
ray originating *inside* a sphere returns `Some(0.0)`. Consequences, in order:

1. `raycast` keeps the nearest hit, and `0.0` can never be beaten → the hit is always the agent itself.
2. `spatial.rs:287` `if t_next > hit.distance { break; }` → with `hit.distance == 0.0` the DDA
   traversal **breaks after the first cell**.
3. `ai/model.rs:947` `if root_agent_id != entity` discards the self-hit and leaves
   `hit_distance = 10.0`, `hit_type = None`.

Net result: **every sensor ray of every agent reports "nothing seen" on every tick.** Food, predators
and prey are all invisible. Measured with a probe (agent at origin, food 4 units ahead):

```
working tree : hit=agent  distance=0      <- blind
HEAD         : hit=food   distance=3.5    <- correct
```

The whole `cargo test --features desktop` suite (93 targets) passes in both states, so no existing
test protects this.

### A1 — revert the inside-sphere branch, keep the degenerate-direction guard

In `src-tauri/src/physics/spatial.rs`, function `intersect_sphere`:

- **KEEP** the new guard `if a <= 0.0 { return None; }`. That one is a genuine improvement.
- **REVERT** the tail of the function so a ray whose near root is behind the origin returns `None`
  again. Delete the `t2` / `Some(0.0)` branch. The tail must read:

```rust
    if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
```

Add a short comment above that `else` explaining why "origin inside the sphere" must **not** be a
hit: the agent carries its own `SpatialCollider` at the ray origin, so returning a hit there makes
`raycast` return the agent itself at distance 0 and blinds every sensor.

### A2 — restore the fixture the previous pass moved

In `src-tauri/tests/physics_tests.rs`, `test_spatial_grid_rebuild_and_raycast`:

- `entity_b` `Position` back to `Vec3::new(99.0, 0.0, 0.0)` (currently `90.0`).
- `ray2` origin back to `Vec3::new(95.0, 0.0, 0.0)` (currently `86.0`).
- The `hit2.distance` expectation stays `3.0`.

With A1 applied this passes again, because the wrapped-boundary ray at case 3 (origin `(99,0,0)`,
which is exactly `entity_b`'s centre) no longer registers a zero-distance self-hit.

### A3 — delete the test that asserts the regression

Delete the whole `#[test] fn test_raycast_inside_collider` from
`src-tauri/tests/physics_tests.rs`. It asserts `hit.distance == 0.0` for a ray starting inside a
collider, i.e. it encodes the bug. This is the one permitted test deletion.

### A4 — add the regression test that would have caught this

Append to `src-tauri/tests/physics_tests.rs`. `ParentAgent` is already imported in that file.

```rust
// The agent carries its own SpatialCollider (`evolution/genotype.rs`) and `ai/model.rs` casts the
// sensor ray from that same Position. If `intersect_sphere` reports a hit for a ray starting inside
// a sphere, `raycast` returns the agent itself at distance 0, `ai/model.rs` discards it as a
// self-collision, and the agent sees nothing — on every ray, every tick, with the whole suite green.
#[test]
fn sensor_ray_from_agent_centre_sees_past_its_own_collider() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(SpatialHashGrid::new_prepopulated(10.0, &bounds));

    let agent = world
        .spawn((Position(Vec3::ZERO), SpatialCollider { radius: 0.5 }))
        .id();
    let food = world
        .spawn((
            Position(Vec3::new(0.0, 0.0, 4.0)),
            SpatialCollider { radius: 0.5 },
        ))
        .id();

    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_spatial_grid_system);
    schedule.run(&mut world);

    let mut system_state: bevy_ecs::system::SystemState<Query<(&Position, &SpatialCollider)>> =
        bevy_ecs::system::SystemState::new(&mut world);
    let query = system_state.get(&world);
    let grid = world.get_resource::<SpatialHashGrid>().unwrap();

    let ray = Ray3D {
        origin: Vec3::ZERO,
        direction: Vec3::new(0.0, 0.0, 1.0),
    };
    let hit = grid
        .raycast(&ray, 10.0, &bounds, &query)
        .expect("the ray should reach the food ahead");

    assert_eq!(
        hit.entity, food,
        "the nearest hit must be the food ahead, not the agent's own collider at the ray origin"
    );
    assert_ne!(hit.entity, agent, "an agent must not sense itself");
    assert!(
        (hit.distance - 3.5).abs() < 1e-3,
        "expected the food surface at 3.5, got {}",
        hit.distance
    );
}
```

---

## B. 45 frontend tests are red — `dispose is not a function`

### Proven cause

`npm run test:frontend` → **45 failed / 401 passed**, every failure one of exactly two lines:

```
37 x  TypeError: oceanMat?.dispose is not a function
21 x  TypeError: g0?.dispose is not a function
```

Only the **R3F reconciler** is mocked (`tests/mocks/react-three-fiber-mock.ts`); **real `three` is
used**. With no reconciler, an r3f *intrinsic* element such as `<bufferGeometry ref={geomRef0} />` or
`<shaderMaterial ref={oceanMaterialRef} />` is rendered by React as a plain **DOM element**, so
`ref.current` is an `HTMLElement`, not a `THREE.BufferGeometry` / `THREE.ShaderMaterial`. `?.` only
guards `null`/`undefined`, not "the property is not callable".

Objects the component creates itself via `useMemo` (`heightMapTexture`, `oceanGeometry`,
`riverData.geom`, and Vegetation's `geometries` / `materials`) **are** real three objects and their
existing `.dispose()` calls are correct — leave those exactly as they are.

### The idiom to follow — it is already in the file you are fixing

`Terrain.tsx:104` guards the same hazard, and the comment at `Terrain.tsx:133-135` explains it
("...plain DOM elements, which have neither `addLevel` nor `setIndex`"):

```ts
if (geomRef0.current && typeof geomRef0.current.setIndex === 'function') {
```

Use that same shape. Do **not** "fix" this by adding a `three` mock, and do **not** weaken it to
`?.dispose?.()` — an unconditional optional call hides a genuinely missing method in production too.

### B1 — `src/components/Landscape/Terrain.tsx`

The cleanup effect at ~line 147 disposes `g0` / `g1` / `g2`, which are refs to `<bufferGeometry>`
intrinsics. Guard each call with a `typeof … === 'function'` check. A small local helper inside the
effect is fine if it reads better than three repeated conditions; give it a one-line comment saying
why the guard exists.

### B2 — `src/components/Landscape/Water.tsx`

Same treatment in the cleanup effect at ~line 691 for the three newly added lines only:
`oceanMat?.dispose()`, `riverMat?.dispose()`, `particles?.geometry?.dispose()` — these are refs to
`<shaderMaterial>` / `<points>` intrinsics. Leave `heightMapTexture.dispose()`,
`oceanGeometry.dispose()` and `riverData.geom.dispose()` untouched.

### B3 — `src/components/Landscape/Vegetation.tsx`

The effect at ~line 485 does `Object.values(geometries).forEach((g) => g.dispose())` with **no guard
at all**. These are `useMemo`-created real three objects passed via `<primitive object={...} />`, so
they normally do have `dispose` — but write the same guard for consistency and so an unmount can
never throw.

---

## C. `PROJECT.md` lost the IPC contract

The working tree deleted 129 lines from `PROJECT.md`, including the whole **"Interface Contracts"**
section — the document `CLAUDE.md` points at with *"read it before changing the IPC surface"* — plus
the Phase 7 ecosystem milestone table (E1–E12). It was replaced with an audit-status summary that
belongs in the audit report, not in the project contract.

1. Restore the file: `git show HEAD:PROJECT.md > PROJECT.md`.
2. The audit inventory that had replaced it (the "Feature Inventory & Audit Tracks" table and the
   "Architecture Overview" list) must not be lost — append it to the existing `AUDIT_REPORT.md`
   under a new `## Audit Track Inventory` heading, if that content is not already there.
3. Do not modify `docs/AUDIT_REPORT.md`.

---

## Definition of done

- `cargo check` (from `src-tauri/`) is clean.
- `npx tsc --noEmit -p tsconfig.json` is clean.
- `PROJECT.md` again contains the "Interface Contracts" heading and the Phase 7 table.
- `git diff --stat` shows changes limited to: `src-tauri/src/physics/spatial.rs`,
  `src-tauri/tests/physics_tests.rs`, `src/components/Landscape/{Terrain,Water,Vegetation}.tsx`,
  `PROJECT.md`, `AUDIT_REPORT.md`.

Report at the end: what you changed per file, and anything you could **not** do. Do not claim a test
suite passed — you were told not to run them.
