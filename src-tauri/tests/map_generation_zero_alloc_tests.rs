mod common;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_erosion_hotpath_zero_heap_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let width = 128;
    let height = 128;
    let seed = 1337;
    let erosion_steps = 2000;

    let mut elevations = vec![0.5f32; width * height];
    let mut flows = vec![0.0f32; width * height];
    let mut rng = StdRng::seed_from_u64(seed as u64);

    // Warm up the RNG and allocations
    for _ in 0..10 {
        let _px: f32 = rng.gen_range(0.0..=(width - 2) as f32);
    }

    // Start tracking allocations
    ALLOCATOR.start_tracking();

    // Exact droplet erosion loop from terrain.rs
    for _ in 0..erosion_steps {
        let mut px: f32 = rng.gen_range(0.0..=(width - 2) as f32);
        let mut py: f32 = rng.gen_range(0.0..=(height - 2) as f32);
        let mut dir_x = 0.0f32;
        let mut dir_y = 0.0f32;
        let mut speed = 1.0f32;
        let mut water = 1.0f32;
        let mut sediment = 0.0f32;

        let inertia = 0.05f32;
        let capacity_coeff = 4.0f32;
        let min_slope = 0.01f32;
        let deposit_speed = 0.3f32;
        let erode_speed = 0.3f32;
        let gravity = 4.0f32;
        let evaporation_rate = 0.05f32;

        for _ in 0..30 {
            // MAX_DROPLET_LIFETIME
            let ipx = px.floor() as usize;
            let ipy = py.floor() as usize;
            if ipx >= width - 1 || ipy >= height - 1 {
                break;
            }

            let tx = px - ipx as f32;
            let ty = py - ipy as f32;

            let h00 = elevations[ipy * width + ipx];
            let h10 = elevations[ipy * width + ipx + 1];
            let h01 = elevations[(ipy + 1) * width + ipx];
            let h11 = elevations[(ipy + 1) * width + ipx + 1];

            let h = h00 * (1.0 - tx) * (1.0 - ty)
                + h10 * tx * (1.0 - ty)
                + h01 * (1.0 - tx) * ty
                + h11 * tx * ty;
            let grad_x = (h10 - h00) * (1.0 - ty) + (h11 - h01) * ty;
            let grad_y = (h01 - h00) * (1.0 - tx) + (h11 - h10) * tx;

            flows[ipy * width + ipx] += water;

            dir_x = dir_x * inertia - grad_x * (1.0 - inertia);
            dir_y = dir_y * inertia - grad_y * (1.0 - inertia);

            let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
            if len > 0.0001 {
                dir_x /= len;
                dir_y /= len;
            } else {
                dir_x = rng.gen_range(-1.0..1.0);
                dir_y = rng.gen_range(-1.0..1.0);
                let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
                if len > 0.0001 {
                    dir_x /= len;
                    dir_y /= len;
                }
            }

            let new_px = px + dir_x;
            let new_py = py + dir_y;

            if new_px < 0.0
                || new_px >= (width - 1) as f32
                || new_py < 0.0
                || new_py >= (height - 1) as f32
            {
                break;
            }

            let new_ipx = new_px.floor() as usize;
            let new_ipy = new_py.floor() as usize;
            let new_tx = new_px - new_ipx as f32;
            let new_ty = new_py - new_ipy as f32;

            let nh00 = elevations[new_ipy * width + new_ipx];
            let nh10 = elevations[new_ipy * width + new_ipx + 1];
            let nh01 = elevations[(new_ipy + 1) * width + new_ipx];
            let nh11 = elevations[(new_ipy + 1) * width + new_ipx + 1];
            let new_h = nh00 * (1.0 - new_tx) * (1.0 - new_ty)
                + nh10 * new_tx * (1.0 - new_ty)
                + nh01 * (1.0 - new_tx) * new_ty
                + nh11 * new_tx * new_ty;

            let delta_h = new_h - h;

            if delta_h > 0.0 {
                let deposit = delta_h.min(sediment);
                sediment -= deposit;

                elevations[ipy * width + ipx] += deposit * (1.0 - tx) * (1.0 - ty);
                elevations[ipy * width + ipx + 1] += deposit * tx * (1.0 - ty);
                elevations[(ipy + 1) * width + ipx] += deposit * (1.0 - tx) * ty;
                elevations[(ipy + 1) * width + ipx + 1] += deposit * tx * ty;
            } else {
                let capacity = (-delta_h).max(min_slope) * speed * water * capacity_coeff;
                if sediment > capacity {
                    let deposit = (sediment - capacity) * deposit_speed;
                    sediment -= deposit;

                    elevations[ipy * width + ipx] += deposit * (1.0 - tx) * (1.0 - ty);
                    elevations[ipy * width + ipx + 1] += deposit * tx * (1.0 - ty);
                    elevations[(ipy + 1) * width + ipx] += deposit * (1.0 - tx) * ty;
                    elevations[(ipy + 1) * width + ipx + 1] += deposit * tx * ty;
                } else {
                    let erode = (capacity - sediment).min(-delta_h) * erode_speed;
                    sediment += erode;

                    elevations[ipy * width + ipx] -= erode * (1.0 - tx) * (1.0 - ty);
                    elevations[ipy * width + ipx + 1] -= erode * tx * (1.0 - ty);
                    elevations[(ipy + 1) * width + ipx] -= erode * (1.0 - tx) * ty;
                    elevations[(ipy + 1) * width + ipx + 1] -= erode * tx * ty;
                }
            }

            speed = (speed * speed + delta_h * delta_h * gravity).sqrt();
            water *= 1.0 - evaporation_rate;

            px = new_px;
            py = new_py;
        }
    }

    let alloc_count = ALLOCATOR.stop_tracking();
    assert_eq!(
        alloc_count, 0,
        "Hydraulic erosion hot path triggered {} heap allocations!",
        alloc_count
    );
}
