//! Card SEL2-E follow-up: a thirst diagnosis after SEL2-E found that more lakes barely reduce
//! dehydration deaths.
//!
//! **Blind to body mass**, with the same allow-list as `sel2e_exploration.rs`; seeds 1–6 are
//! exploratory.
//!
//! Grid: 100 founders, 8 lakes, `prey_cap` 320, no plant reserve, `resource_capacity_scale` ∈
//! {1.0, 0.7} × `live.dehydration_ticks` ∈ {120, 240, 480, 960} × seeds 1–6 = 48 runs of 18,000
//! ticks. Everything else is the SEL2-E base world. Question: does a longer dehydration grace move
//! deaths from thirst to starvation, and how many generations does a run then reach?
//!
//! ```text
//! cargo run --release --example sel2e_thirst_probe > sel2e_thirst_probe.tsv
//! ```

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{live_observables, LiveExperimentAdapter};
use std::sync::{Arc, Mutex};

const TICKS: u64 = 18_000;
const SAMPLE_EVERY: u64 = 50;
const SEEDS: [u64; 6] = [1, 2, 3, 4, 5, 6];
const SCALES: [f64; 2] = [1.0, 0.7];
const GRACES: [f64; 4] = [120.0, 240.0, 480.0, 960.0];
const THREADS: usize = 8;
/// The only observables this driver reads. Nothing about body mass.
const ALLOWED: [&str; 7] = [
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.deaths_starvation",
    "live.deaths_dehydration",
];

#[derive(Clone, Copy, Debug)]
struct Job {
    scale: f64,
    grace: f64,
    seed: u64,
}

#[derive(Clone, Debug, Default)]
struct Stats {
    min_agents: f64,
    mean_agents: f64,
    final_agents: f64,
    crop_first: f64,
    crop_second_half: f64,
    births: f64,
    deaths: f64,
    starvation: f64,
    dehydration: f64,
    lifespan_weighted: f64,
}

fn run(job: Job) -> Stats {
    let initial = InitialConditionSet::new(
        [
            ("live.founders", 100.0),
            ("live.predator_fraction", 0.0),
            ("live.trees", 8.0),
            ("live.lakes", 8.0),
            ("live.mortality", 1.0),
            ("live.reproduction", 1.0),
            ("live.locomotion", 1.0),
            ("live.drive_floor", 0.5),
            ("live.thirst_steering", 1.0),
            ("live.prey_cap", 320.0),
            ("live.resource_capacity_scale", job.scale),
            ("live.dehydration_ticks", job.grace),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect(),
    );
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        job.seed,
        (16, 16),
        TICKS,
    )
    .expect("the declared world builds");
    let read = |a: &mut LiveExperimentAdapter| -> Vec<(String, f64)> {
        let mut w = a.world();
        live_observables(&mut w)
            .into_iter()
            .filter(|(k, _)| ALLOWED.contains(&k.as_str()))
            .collect()
    };
    let get = |obs: &[(String, f64)], id: &str| {
        obs.iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    };
    let mut st = Stats {
        min_agents: f64::MAX,
        crop_first: get(&read(&mut a), "live.standing_crop"),
        ..Default::default()
    };
    let (mut agents_sum, mut agents_n, mut crop_sum, mut crop_n) = (0.0, 0u64, 0.0, 0u64);
    for tick in 1..=TICKS {
        a.run_schedule_once();
        let obs = read(&mut a);
        let d = get(&obs, "live.deaths");
        st.births += get(&obs, "live.births");
        st.deaths += d;
        st.starvation += get(&obs, "live.deaths_starvation");
        st.dehydration += get(&obs, "live.deaths_dehydration");
        st.lifespan_weighted += get(&obs, "live.mean_lifespan") * d;
        if tick % SAMPLE_EVERY == 0 {
            let n = get(&obs, "live.agent_count");
            st.min_agents = st.min_agents.min(n);
            st.final_agents = n;
            agents_sum += n;
            agents_n += 1;
            if tick > TICKS / 2 {
                crop_sum += get(&obs, "live.standing_crop");
                crop_n += 1;
            }
        }
    }
    st.mean_agents = agents_sum / agents_n.max(1) as f64;
    st.crop_second_half = crop_sum / crop_n.max(1) as f64;
    st
}

fn main() {
    let mut jobs = Vec::new();
    for &seed in &SEEDS {
        for &scale in &SCALES {
            for &grace in &GRACES {
                jobs.push(Job { scale, grace, seed });
            }
        }
    }
    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs));
    let results = Arc::new(Mutex::new(Vec::with_capacity(total)));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let (queue, results) = (Arc::clone(&queue), Arc::clone(&results));
            std::thread::spawn(move || loop {
                let job = queue.lock().unwrap().pop();
                let Some(job) = job else { break };
                let st = run(job);
                let mut out = results.lock().unwrap();
                out.push((job, st));
                eprintln!("done {}/{}", out.len(), total);
            })
        })
        .collect();
    for h in handles {
        h.join().expect("a worker thread panicked");
    }
    let mut all = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    all.sort_by(|(a, _), (b, _)| {
        (a.scale, a.grace, a.seed)
            .partial_cmp(&(b.scale, b.grace, b.seed))
            .unwrap()
    });
    println!("scale\tgrace\tseed\tmin_agents\tmean_agents\tfinal_agents\tcrop_ratio_to_start\tbirths\tdeaths\tstarvation\tdehydration\tmean_lifespan\tgen_by_lifespan\tgen_by_turnover");
    for (j, s) in &all {
        let l = if s.deaths > 0.0 {
            s.lifespan_weighted / s.deaths
        } else {
            f64::NAN
        };
        println!(
            "{}\t{}\t{}\t{}\t{:.1}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{:.0}\t{:.1}\t{:.1}",
            j.scale,
            j.grace,
            j.seed,
            s.min_agents,
            s.mean_agents,
            s.final_agents,
            s.crop_second_half / s.crop_first,
            s.births,
            s.deaths,
            s.starvation,
            s.dehydration,
            l,
            TICKS as f64 / l,
            s.births / s.mean_agents.max(1.0)
        );
    }
    println!();
    println!("SUMMARY scale\tgrace\tshare_starvation\tmin_agents_min\textinct\tmean_agents_median\tmean_lifespan_pooled\tgen_min..max\tcrop_ratio_min..max");
    for &scale in &SCALES {
        for &grace in &GRACES {
            let rows: Vec<&(Job, Stats)> = all
                .iter()
                .filter(|(j, _)| j.scale == scale && j.grace == grace)
                .collect();
            let (st, de, dd, lw) = rows.iter().fold((0.0, 0.0, 0.0, 0.0), |acc, (_, s)| {
                (
                    acc.0 + s.starvation,
                    acc.1 + s.dehydration,
                    acc.2 + s.deaths,
                    acc.3 + s.lifespan_weighted,
                )
            });
            let min_pop = rows
                .iter()
                .map(|(_, s)| s.min_agents)
                .fold(f64::MAX, f64::min);
            let extinct = rows.iter().filter(|(_, s)| s.final_agents == 0.0).count();
            let mut means: Vec<f64> = rows.iter().map(|(_, s)| s.mean_agents).collect();
            means.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = (means[2] + means[3]) / 2.0;
            let gens: Vec<f64> = rows
                .iter()
                .map(|(_, s)| {
                    let l = s.lifespan_weighted / s.deaths.max(1.0);
                    (TICKS as f64 / l).min(s.births / s.mean_agents.max(1.0))
                })
                .collect();
            let crops: Vec<f64> = rows
                .iter()
                .map(|(_, s)| s.crop_second_half / s.crop_first)
                .collect();
            println!(
                "SUMMARY {scale}\t{grace}\t{:.3}\t{min_pop}\t{extinct}/6\t{median:.1}\t{:.0}\t{:.1}..{:.1}\t{:.3}..{:.3}",
                st / (st + de).max(1.0),
                lw / dd.max(1.0),
                gens.iter().cloned().fold(f64::MAX, f64::min),
                gens.iter().cloned().fold(f64::MIN, f64::max),
                crops.iter().cloned().fold(f64::MAX, f64::min),
                crops.iter().cloned().fold(f64::MIN, f64::max),
            );
        }
    }
}
