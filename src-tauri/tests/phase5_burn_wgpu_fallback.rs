// The GPU fallback path only exists with the `ml-wgpu` feature (G2); without it there is no Wgpu
// backend to fall back FROM, so this suite has nothing to assert.
#![cfg(feature = "ml-wgpu")]

use anima_engine_lib::ai::hrrl::Transition;
use anima_engine_lib::ai::model::{BrainModel, BrainModelBackend};
use anima_engine_lib::core::thread_supervisor::{ThreadSupervisor, LEARN};
use anima_engine_lib::core::training::{
    spawn_ndarray_learner, spawn_wgpu_learner, ModelUpdate, ModelUpdateDiagnostics, ACTION_DIM,
    BATCH_SIZE, STATE_DIM,
};
use burn::tensor::{Tensor, TensorData};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

fn assert_model_outputs_in_bounds(brain_model: &BrainModel, step: usize) {
    let input_data = TensorData::new(vec![0.25 + (step as f32) * 0.01; STATE_DIM], [1, STATE_DIM]);
    let actor_out = match brain_model.backend() {
        BrainModelBackend::Wgpu(model, device) | BrainModelBackend::NdArray(model, device) => {
            let input_tensor = Tensor::<2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            actor_out
        }
    };
    let actions = actor_out.into_data().try_into_vec::<f32>().unwrap();
    assert_eq!(actions.len(), ACTION_DIM);
    for action in actions {
        assert!(
            action.is_finite() && (0.0..=1.0).contains(&action),
            "Action {action} at update {step} must be finite and in [0.0, 1.0]"
        );
    }
}

#[test]
fn test_wgpu_learner_publishes_synced_models_across_threads() {
    let _lock = TEST_LOCK.lock().unwrap();
    std::env::set_var("ANIMA_USE_GPU", "true");

    let mut brain_model = BrainModel::new(STATE_DIM, 64, ACTION_DIM);
    let using_wgpu = matches!(brain_model.backend(), BrainModelBackend::Wgpu(..));

    let running = Arc::new(AtomicBool::new(true));
    let (trans_tx, trans_rx) = crossbeam_channel::bounded(256);
    let (model_tx, model_rx) = crossbeam_channel::bounded(1);
    let (old_model_tx, old_model_rx) = crossbeam_channel::bounded(4);
    let diagnostics = ModelUpdateDiagnostics::default();
    let (_checkpoint_tx, checkpoint_rx) = crossbeam_channel::bounded(1);
    let supervisor = ThreadSupervisor::new();
    let args = (
        Arc::clone(&running),
        trans_rx,
        model_tx,
        old_model_rx,
        diagnostics.clone(),
        424242,
        None,
        checkpoint_rx,
    );

    let learner = if using_wgpu {
        spawn_wgpu_learner(args, supervisor.token(LEARN))
    } else {
        spawn_ndarray_learner(args, supervisor.token(LEARN))
    };

    let consumer_running = Arc::clone(&running);
    let consumer = std::thread::spawn(move || {
        let mut updates = 0usize;
        let deadline = Instant::now() + Duration::from_secs(60);
        while updates < 32 && Instant::now() < deadline && consumer_running.load(Ordering::SeqCst) {
            match model_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(ModelUpdate::Wgpu(new_model)) => {
                    if let Some(old) = brain_model.replace_wgpu_model(new_model) {
                        let _ = old_model_tx.try_send(ModelUpdate::Wgpu(old));
                    }
                    assert_model_outputs_in_bounds(&brain_model, updates);
                    updates += 1;
                }
                Ok(ModelUpdate::NdArray(new_model)) => {
                    if let Some(old) = brain_model.replace_ndarray_model(new_model) {
                        let _ = old_model_tx.try_send(ModelUpdate::NdArray(old));
                    }
                    assert_model_outputs_in_bounds(&brain_model, updates);
                    updates += 1;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
        }
        updates
    });

    for i in 0..(BATCH_SIZE * 40) {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let s = (i % 7) as f32 * 0.1;
        let _ = trans_tx.send_timeout(
            Transition {
                state: [s; STATE_DIM],
                action: [0.5; ACTION_DIM],
                reward: 0.25,
                next_state: [s + 0.05; STATE_DIM],
            },
            Duration::from_secs(2),
        );
    }

    let deadline = Instant::now() + Duration::from_secs(60);
    while diagnostics.snapshot().published < 32 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }

    running.store(false, Ordering::SeqCst);
    drop(trans_tx);
    let received_updates = consumer.join().expect("consumer thread must not panic");
    learner.join().expect("learner thread must not panic");

    assert!(
        received_updates >= 32,
        "expected >= 32 synced cross-thread model updates, got {received_updates} (wgpu={using_wgpu})"
    );
}

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_wgpu_fallback_to_ndarray_and_valid_actions() {
    let _lock = TEST_LOCK.lock().unwrap();

    // 1. Test Flex CPU Fallback (Forced via environment variable)
    std::env::set_var("ANIMA_USE_GPU", "false");
    let brain_model_cpu = BrainModel::new(15, 64, 4);

    // Assert that fallback chose NdArray (burn-flex CPU)
    match brain_model_cpu.backend() {
        BrainModelBackend::NdArray(model, device) => {
            // Run inference to verify valid actions in [0.0, 1.0]
            let input_data = TensorData::new(vec![0.5; 15], [1, 15]);
            let input_tensor = Tensor::<2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            let actions = actor_out.into_data().try_into_vec::<f32>().unwrap();
            assert_eq!(actions.len(), 4);
            for action in actions {
                assert!(
                    (0.0..=1.0).contains(&action),
                    "Action {} out of bounds [0.0, 1.0]",
                    action
                );
            }
        }
        _ => panic!("Expected BrainModel to fall back to NdArray when ANIMA_USE_GPU=false"),
    }

    // 2. Test Wgpu Initialization / Fallback
    std::env::set_var("ANIMA_USE_GPU", "true");
    let brain_model_gpu = BrainModel::new(15, 64, 4);

    match brain_model_gpu.backend() {
        BrainModelBackend::Wgpu(model, device) => {
            println!("Successfully initialized WGPU backend.");
            let input_data = TensorData::new(vec![0.5; 15], [1, 15]);
            let input_tensor = Tensor::<2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            let actions = actor_out.into_data().try_into_vec::<f32>().unwrap();
            assert_eq!(actions.len(), 4);
            for action in actions {
                assert!(
                    (0.0..=1.0).contains(&action),
                    "Action {} out of bounds [0.0, 1.0]",
                    action
                );
            }
        }
        BrainModelBackend::NdArray(model, device) => {
            println!("WGPU failed/unsupported on this machine, gracefully fell back to CPU flex.");
            let input_data = TensorData::new(vec![0.5; 15], [1, 15]);
            let input_tensor = Tensor::<2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            let actions = actor_out.into_data().try_into_vec::<f32>().unwrap();
            assert_eq!(actions.len(), 4);
            for action in actions {
                assert!(
                    (0.0..=1.0).contains(&action),
                    "Action {} out of bounds [0.0, 1.0]",
                    action
                );
            }
        }
    }
}
