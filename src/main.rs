#![forbid(unsafe_code)]
//! main.rs
//! Binary entry point for hix_heatmap_indices_for_compute (POC-1).
//!
//! This file is the Tier-3 ("macro") recovery level for a one-shot CLI:
//! any error that propagates up to here is reported as a numeric code only
//! (no paths, no user data), appended to an error log next to the
//! executable, and the process exits with a non-zero status. No panic.

/*
# one-time: third-party tools (unavoidable for v1)
```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
cargo install rustfilt
```

# build the analyzer
cd /home/YOU/code/hix_heatmap_indices_for_compute && cargo test && cargo build --release

# e.g. Pumpkin:
```bash
cd /home/YOU/code/Pumpkin
cargo llvm-cov clean --workspace
cargo llvm-cov run --release --no-report --bin pumpkin
#   ... exercise the server with a written-down scenario + duration,
#   ... then stop it using console `stop`, so the .profraw is written
cargo llvm-cov report --release --lcov --output-path /home/YOU/hix/pumpkin_run1.lcov
e.g.
```bash
cargo llvm-cov report --release --lcov --output-path /home/oops/code/hix_heatmap_indices_for_compute/run_data/pumpkin_run3.lcov
```

<pseudo-code>
/home/YOU/code/hix_heatmap_indices_for_compute/target/release/hix_heatmap_indices_for_compute \
  --lcov /home/oops/code/hix_heatmap_indices_for_compute/run_data/pumpkin_run1.lcov \
  --out-dir /home/YOU/hix_heatmap_indices_for_compute/run_data/run1 \
  --top-n 30

e.g.
```bash
cargo run --release -- \
  --lcov /home/oops/code/hix_heatmap_indices_for_compute/run_data/pumpkin_run3.lcov \
  --out-dir /home/oops/code/hix_heatmap_indices_for_compute/run_data/run3 \
  --top-n 15
```

# demangle .txt and .tsv (NOT .lcov)
```
rustfilt < /home/oops/code/hix_heatmap_indices_for_compute/run_data/run1/hix_heatmap_indices_for_compute_report.txt \
        > /home/oops/code/hix_heatmap_indices_for_compute/run_data/run1/hix_heatmap_indices_for_compute_report.demangled.txt
```
```
rustfilt < /home/oops/code/hix_heatmap_indices_for_compute/run_data/run1/hix_heatmap_indices_for_compute_sites.tsv \
        > /home/oops/code/hix_heatmap_indices_for_compute/run_data/run1/hix_heatmap_indices_for_compute_sites.demangled.tsv
```

  */

mod hix_heatmap_indices_for_compute;
use hix_heatmap_indices_for_compute::{append_error_code_to_log, run_heatmap_cli, USAGE_TEXT};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run_heatmap_cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_code) => {
            // Numeric code only: safe in all modes (no heap, no PII).
            eprintln!(
                "hix_heatmap_indices_for_compute: failed with error code {}",
                error_code.code()
            );

            // Debug mode: human-readable description (Display exists only in debug/test).
            #[cfg(debug_assertions)]
            eprintln!(
                "hix_heatmap_indices_for_compute: debug detail: {}",
                error_code
            );

            // Command-line mistakes are user-fixable: show usage text (static, no heap).
            if error_code.is_cli_usage_error() {
                eprintln!("{}", USAGE_TEXT);
            }

            // Log the code; if logging itself fails, say so with its own code.
            if let Err(log_error_code) = append_error_code_to_log(error_code) {
                eprintln!(
                    "hix_heatmap_indices_for_compute: error log unavailable, error code {}",
                    log_error_code.code()
                );
            }
            ExitCode::FAILURE
        }
    }
}
