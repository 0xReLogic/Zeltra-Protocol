//! DEC-049 Gate V-3 Statistical Dudect Verification Tool ($N \ge 10^6$ traces)
//!
//! Executes Welch's two-tailed t-test over >= 1,000,000 traces to verify that
//! ConstantTimeMemoScanner::trial_decrypt_ct satisfies |t| < 4.5.

use rand::rngs::OsRng;
use rand::RngCore;
use std::env;

use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use zeltra_core::keys::IncomingViewingKey;
use zeltra_core::memo::{encrypt_committing_note_memo, ENCRYPTED_MEMO_SIZE};
use zeltra_core::memo_ct::trial_decrypt_ct;

#[derive(Clone, Copy, Debug, Default)]
pub struct WelfordStats {
    pub count: usize,
    pub mean: f64,
    pub m2: f64,
}

impl WelfordStats {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    pub fn update(&mut self, x: f64) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }
}

pub fn welch_t_statistic(g0: &WelfordStats, g1: &WelfordStats) -> f64 {
    if g0.count < 2 || g1.count < 2 {
        return 0.0;
    }
    let var0 = g0.variance();
    let var1 = g1.variance();
    let se = (var0 / g0.count as f64 + var1 / g1.count as f64).sqrt();
    if se == 0.0 {
        0.0
    } else {
        (g0.mean - g1.mean) / se
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn measure_trace_cycles<F: FnOnce() -> R, R>(f: F) -> u64 {
    core::arch::x86_64::_mm_lfence();
    let start = core::arch::x86_64::_rdtsc();
    core::arch::x86_64::_mm_lfence();
    let result = std::hint::black_box(f());
    core::arch::x86_64::_mm_lfence();
    let end = core::arch::x86_64::_rdtsc();
    core::arch::x86_64::_mm_lfence();
    std::hint::black_box(result);
    end.wrapping_sub(start)
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
fn measure_trace_cycles<F: FnOnce() -> R, R>(f: F) -> u64 {
    let start = std::time::Instant::now();
    let result = std::hint::black_box(f());
    let elapsed = start.elapsed().as_nanos() as u64;
    std::hint::black_box(result);
    elapsed
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let n_traces: usize = if args.len() > 1 {
        args[1].parse().unwrap_or(1_000_000)
    } else {
        1_000_000
    };

    println!("═══════════════════════════════════════════════════════════════════════");
    println!("  DEC-049 GATE V-3: STATISTICAL DUDECT VERIFICATION (N = {})", n_traces);
    println!("═══════════════════════════════════════════════════════════════════════");

    let mut rng = OsRng;
    let mut ivk_bytes = [0u8; 32];
    rng.fill_bytes(&mut ivk_bytes);
    let ivk = IncomingViewingKey::from_bytes(&ivk_bytes);

    let num_pool = 32usize;
    let mut valid_memos = Vec::with_capacity(num_pool);
    for i in 0..num_pool {
        let rho = Fr::rand(&mut rng);
        let (memo, _) =
            encrypt_committing_note_memo((i as u64 + 1) * 10_000, rho, (i % 2) as u64, &ivk)
                .unwrap();
        let arr: [u8; ENCRYPTED_MEMO_SIZE] = memo.try_into().unwrap();
        valid_memos.push(arr);
    }

    let mut random_memos = Vec::with_capacity(num_pool);
    for _ in 0..num_pool {
        let mut r = [0u8; ENCRYPTED_MEMO_SIZE];
        rng.fill_bytes(&mut r);
        random_memos.push(r);
    }

    let mut valid_stats = WelfordStats::new();
    let mut random_stats = WelfordStats::new();

    let start_time = std::time::Instant::now();

    for i in 0..n_traces {
        let v = &valid_memos[i % num_pool];
        let r = &random_memos[i % num_pool];

        if i % 2 == 0 {
            let cv = unsafe { measure_trace_cycles(|| trial_decrypt_ct(v, &ivk_bytes)) };
            valid_stats.update(cv as f64);

            let cr = unsafe { measure_trace_cycles(|| trial_decrypt_ct(r, &ivk_bytes)) };
            random_stats.update(cr as f64);
        } else {
            let cr = unsafe { measure_trace_cycles(|| trial_decrypt_ct(r, &ivk_bytes)) };
            random_stats.update(cr as f64);

            let cv = unsafe { measure_trace_cycles(|| trial_decrypt_ct(v, &ivk_bytes)) };
            valid_stats.update(cv as f64);
        }

        if (i + 1) % 100_000 == 0 || (i + 1) == n_traces {
            let t = welch_t_statistic(&valid_stats, &random_stats);
            println!(
                "[{:7} / {}] Current |t| = {:.4} (Threshold: < 4.5)",
                i + 1,
                n_traces,
                t.abs()
            );
        }
    }

    let t_final = welch_t_statistic(&valid_stats, &random_stats);
    let elapsed = start_time.elapsed();

    println!("───────────────────────────────────────────────────────────────────────");
    println!("Evaluation Completed in: {:?}", elapsed);
    println!("Valid Memos:   Mean = {:.2} cycles, Variance = {:.2}", valid_stats.mean, valid_stats.variance());
    println!("Random Memos:  Mean = {:.2} cycles, Variance = {:.2}", random_stats.mean, random_stats.variance());
    println!("Final Welch's t-statistic: {:.4}", t_final);
    println!("───────────────────────────────────────────────────────────────────────");

    if t_final.abs() < 4.5 {
        println!(">>> GATE V-3 VERIFICATION PASSED: |t| = {:.4} < 4.5 (Constant-Time Verified!) <<<", t_final.abs());
    } else {
        panic!(">>> GATE V-3 FAILED: |t| = {:.4} >= 4.5 <<<", t_final.abs());
    }
}
