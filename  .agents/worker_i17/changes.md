# Changes - Milestone I17: Predator-Prey Dynamics & Ecological Combat

## Implementation Summary

1. **Predator and Prey Component Tags**:
   - Defined `Predator` and `Prey` component tags in `src-tauri/src/core/ecs.rs`.
   - Defined `AgentClass` serialization enum (`Predator`, `Prey`).

2. **Ecological Spawning & Population Split**:
   - Extended `SpawnGenotypeCommand` in `src-tauri/src/core/engine.rs` to accept an `agent_class` argument and insert the corresponding component tag on the spawned root entity.
   - Initialized the ecosystem by splitting the starting population (3 Predators, 7 Prey) during startup initialization in `src-tauri/src/core/engine.rs`.

3. **Evolutionary Role Conservation**:
   - Updated `apply_staggered_evolution_system` in `src-tauri/src/core/engine.rs` to read the component tag (`Predator` vs `Prey`) of the old entity being replaced, passing the same class configuration to `SpawnGenotypeCommand` so agent roles are conserved across generations.

4. **Neural Network Branching Target Selection**:
   - Updated `brain_inference_system` in `src-tauri/src/ai/model.rs`:
     - Prey target the nearest active `Food` node.
     - Predators target the nearest active `Prey` agent (`energy > 0.0`).
     - Transformed target relative coordinates into the agent's local space and wrote them to the first 3 input dimensions of the neural network vector, maintaining the 9-dimensional format.
   - Refined `hrrl_learning_system` in `src-tauri/src/ai/model.rs` to also branch targets similarly.
   - Resolved a Bevy ECS borrow check query conflict in `hrrl_learning_system` using `ParamSet` and copying Prey data into a zero-allocation, stack-allocated array before the main update loop.

5. **Ecological Combat System**:
   - Implemented `combat_system` in `src-tauri/src/core/ecs.rs` and registered it in the Bevy schedule (`combat_system.after(integrate_physics_system)`) in `src-tauri/src/core/engine.rs`.
   - Deduces energy from Prey (up to Prey's energy, reducing it to 0.0 if dead) and transfers it to the colliding Predator (up to predator's `energy_target`) when the distance between their centroids is `< 1.5`.

6. **Depletion Freezing Logic**:
   - Refined `integrate_physics_system` in `src-tauri/src/physics/dynamics.rs` to freeze the physical movement (velocity and force set to zero) of:
     - Root Prey agents whose energy is depleted (`<= 0.0`).
     - Child segments whose parent agent is depleted.
   - This ensures that depleted prey freeze automatically, while keeping generic test agents in physics and simulation loops functional.

7. **Predator Metabolic Rate**:
   - Configured Predators to have a higher base metabolic rate (`k_base = 0.2` instead of `0.1`) in `metabolic_decay_system` in `src-tauri/src/core/ecs.rs` to incentivize active hunting.

8. **Convenience & Fixes**:
   - Derived `Default` on `LastTransitionState` in `src-tauri/src/ai/hrrl.rs` to enable clean default state initialization in tests.
   - Fixed a double mutable borrow issue on the pheromone grid in `src-tauri/src/ai/pheromone.rs`.

## Verification & Test Results

- Created a comprehensive test suite in `src-tauri/tests/predator_prey_combat_tests.rs`:
  - `test_predator_prey_classification`: Spawns predator/prey and verifies they receive correct Bevy tags.
  - `test_predator_tracking_inputs`: Verifies neural network targets branch (Prey -> Food, Predator -> Prey) and write correct relative coords.
  - `test_predator_prey_collision_and_combat`: Verifies collision transfers energy and reduces prey's energy.
  - `test_prey_carcass_freezing`: Verifies that root prey and child segments freeze once energy is <= 0.0.
  - `test_zero_allocation_combat_hot_path`: Verifies that the combat system performs zero heap allocations on the hot path.
- Verified that all 64 workspace tests compile and pass sequentially (`cargo test -j 1`).
- Confirmed that there are no Clippy warnings (`cargo clippy --tests`).
