//! G2 gate #1, for the laws that have moved: a law expressed **once** in `anima-domain` is the same
//! law on both sides of the engine boundary.
//!
//! The gate's wording is "one law change, expressed once, observably alters both the headless runner
//! and the live world". These tests do not prove that for every law — `WorldLawSet` and
//! `ExoticEnergyLaw` are still inside the AE module cycle. They prove it for the ones extracted so
//! far, and they are written so that *re-introducing* a per-engine copy fails them.
//!
//! The failure they guard against is specific and has happened before in this codebase: two
//! subsystems each defining their own constant, agreeing at first, and drifting apart later with
//! nothing to notice.

use anima_engine_lib::core::determinism::tick_timestamp_ms;
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::sim_rules::{TICKS_PER_YEAR, TICK_HZ};

/// The names the engine reads resolve to the domain crate's values.
///
/// This is the weaker half of the gate, and worth being precise about what it does and does not
/// show. It catches a copy that has **drifted** — the moment a second definition stops saying 60.0,
/// this fails. It does not catch the copy on the day it is written, because two separate constants
/// both holding 60.0 compare equal, and that is exactly the state the gate wants to forbid: a
/// duplicate agrees at first and diverges later.
///
/// Closing that gap is [`no_engine_module_redeclares_a_law_the_domain_crate_owns`], below.
#[test]
fn the_engine_reads_the_domain_crates_values_for_the_time_laws() {
    assert_eq!(TICK_HZ, anima_domain::laws::TICK_HZ);
    assert_eq!(TICKS_PER_YEAR, anima_domain::laws::TICKS_PER_YEAR);
    assert_eq!(
        anima_engine_lib::core::sim_rules::TICK_DT_SECONDS,
        anima_domain::laws::TICK_DT_SECONDS
    );
    assert_eq!(
        anima_engine_lib::core::sim_rules::TICKS_PER_EPOCH,
        anima_domain::laws::TICKS_PER_EPOCH
    );
    assert_eq!(
        anima_engine_lib::core::sim_rules::SECONDS_PER_YEAR,
        anima_domain::laws::SECONDS_PER_YEAR
    );
}

/// The live world and the headless scheduler derive from the SAME tick rate.
///
/// `tick_timestamp_ms` is the live path (G1.3 stamps the chronicle with it). `SimClock::fires` is
/// the headless path (M2 paces experiment bands with it). Both are computed here from the one
/// constant in `anima-domain`, so a change to that constant moves both — which is the gate.
#[test]
fn one_tick_rate_paces_both_the_live_clock_and_the_headless_scheduler() {
    // Live: one simulated second of ticks must advance the derived timestamp by exactly 1000 ms.
    let ticks_per_second = TICK_HZ as u64;
    let elapsed_ms = tick_timestamp_ms(ticks_per_second) - tick_timestamp_ms(0);
    assert_eq!(
        elapsed_ms, 1000,
        "the live timestamp derivation disagrees with TICK_HZ"
    );

    // Headless: a band whose period is one simulated second fires exactly once per second of ticks.
    let fires = (1..=ticks_per_second)
        .filter(|t| SimClock::fires(*t, ticks_per_second))
        .count();
    assert_eq!(
        fires, 1,
        "the headless scheduler disagrees with TICK_HZ about how long a second is"
    );

    // And the two agree with each other, which is the property the gate is actually about.
    assert_eq!(
        SimClock::fire_count(TICKS_PER_YEAR, ticks_per_second),
        (TICKS_PER_YEAR / ticks_per_second),
        "a year's worth of per-second bands must match the declared year length"
    );
}

/// The unit vocabulary is shared too: MU is not EU, expressed once (ER04).
#[test]
fn the_unit_vocabulary_is_shared_not_duplicated() {
    use anima_engine_lib::core::exotic_energy::{UnitId, EU_UNIT, MU_UNIT};
    assert_eq!(EU_UNIT, anima_domain::units::EU_UNIT);
    assert_eq!(MU_UNIT, anima_domain::units::MU_UNIT);
    assert!(UnitId::new(EU_UNIT).is_eu());
    assert!(
        !UnitId::new(MU_UNIT).is_eu(),
        "an exotic unit must never read as the closed-energy unit"
    );
}

/// No module in the engine crate declares a constant the domain crate owns.
///
/// This is the half that catches a duplicate the day it is written rather than the day it drifts.
///
/// It is a source scan, and that deserves a justification rather than an apology. The direct check
/// would be to compare addresses — one definition, one address — but these laws are `const`, not
/// `static`, so they have no address of their own: a `const` is inlined at each use site. Promoting
/// them to `static` to make the trick available would give up their use in const contexts, and buy
/// a guarantee Rust does not actually make, since distinct statics are not *promised* distinct
/// addresses. The property being asserted is structural, so it is asserted where it is structurally
/// visible: a redeclaration is a `const NAME:` or `static NAME:` binding in this crate's sources,
/// and a `pub use` re-export is not.
///
/// Same shape as G2 gate #2, which greps `cargo tree` rather than trusting that a build succeeding
/// means the gated crates are gone. Compilation alone is not evidence about structure.
#[test]
fn no_engine_module_redeclares_a_law_the_domain_crate_owns() {
    use std::path::{Path, PathBuf};

    /// Every law `anima-domain` is the sole owner of. Adding a law to `laws.rs` means adding it
    /// here, or it ships without this protection.
    const OWNED_BY_THE_DOMAIN: [&str; 5] = [
        "TICK_HZ",
        "TICK_DT_SECONDS",
        "TICKS_PER_EPOCH",
        "SECONDS_PER_YEAR",
        "TICKS_PER_YEAR",
    ];

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect(&src, &mut files);
    // A scan that silently found nothing would report "clean" forever — the one way this test can
    // rot into a no-op is by looking in the wrong place.
    assert!(
        files.len() > 20,
        "the scan found only {} files under {}; the path is wrong, not the tree clean",
        files.len(),
        src.display()
    );

    let mut offenders = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            // Drop line comments, so prose *about* a constant is not mistaken for one.
            let code = line.split("//").next().unwrap_or("");
            for name in OWNED_BY_THE_DOMAIN {
                // The trailing colon is what separates a declaration (`const TICK_HZ: f64 = ..`)
                // from a mention, and keeps `TICK_HZ` from matching a longer name.
                if code.contains(&format!("const {name}:"))
                    || code.contains(&format!("static {name}:"))
                {
                    offenders.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the engine redeclares a law that `anima-domain` owns. Re-export it from \
         `anima_domain::laws` instead — a second definition agrees today and drifts later, with \
         nothing to notice:\n{}",
        offenders.join("\n")
    );
}
