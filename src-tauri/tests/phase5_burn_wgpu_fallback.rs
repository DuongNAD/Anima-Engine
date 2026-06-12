use std::sync::Mutex;
use burn::tensor::{Tensor, Data, Shape};
use anima_engine_lib::ai::model::{BrainModel, BrainModelBackend};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_wgpu_fallback_to_ndarray_and_valid_actions() {
    let _lock = TEST_LOCK.lock().unwrap();

    // 1. Test NdArray CPU Fallback (Forced via environment variable)
    std::env::set_var("ANIMA_USE_GPU", "false");
    let brain_model_cpu = BrainModel::new(15, 64, 4);
    
    // Assert that fallback chose NdArray CPU
    match &brain_model_cpu.backend {
        BrainModelBackend::NdArray(model, device) => {
            // Run inference to verify valid actions in [0.0, 1.0]
            let input_data = Data::new(vec![0.5; 15], Shape::new([1, 15]));
            let input_tensor = Tensor::<burn_ndarray::NdArray<f32>, 2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            let actions = actor_out.into_data().value;
            assert_eq!(actions.len(), 4);
            for action in actions {
                assert!((0.0..=1.0).contains(&action), "Action {} out of bounds [0.0, 1.0]", action);
            }
        }
        _ => panic!("Expected BrainModel to fall back to NdArray when ANIMA_USE_GPU=false"),
    }

    // 2. Test Wgpu Initialization / Fallback
    std::env::set_var("ANIMA_USE_GPU", "true");
    let brain_model_gpu = BrainModel::new(15, 64, 4);
    
    match &brain_model_gpu.backend {
        BrainModelBackend::Wgpu(model, device) => {
            println!("Successfully initialized WGPU backend.");
            let input_data = Data::new(vec![0.5; 15], Shape::new([1, 15]));
            let input_tensor = Tensor::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>, 2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            let actions = actor_out.into_data().value;
            assert_eq!(actions.len(), 4);
            for action in actions {
                assert!((0.0..=1.0).contains(&action), "Action {} out of bounds [0.0, 1.0]", action);
            }
        }
        BrainModelBackend::NdArray(model, device) => {
            println!("WGPU failed/unsupported on this machine, gracefully fell back to CPU NdArray.");
            let input_data = Data::new(vec![0.5; 15], Shape::new([1, 15]));
            let input_tensor = Tensor::<burn_ndarray::NdArray<f32>, 2>::from_data(input_data, device);
            let (actor_out, _) = model.forward(input_tensor);
            let actions = actor_out.into_data().value;
            assert_eq!(actions.len(), 4);
            for action in actions {
                assert!((0.0..=1.0).contains(&action), "Action {} out of bounds [0.0, 1.0]", action);
            }
        }
    }
}
