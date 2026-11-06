//! # hix_heatmap_indices_for_compute.rs — POC-1: Primary Count-Heat (first slice of MVP 1.1)
//!
//! ## Development phase & use-type
//! - Phase: POC-1. First runnable slice of MVP 1.1 ("Primary Count").
//! - Use-type: local developer analysis tool, run on demand, one-shot CLI.
//! - Priorities: transparent, deterministic, reproducible numbers;
//!   analysis that scales with sanity (no AST, no type analysis);
//!   std-only (no crates) so this single flat module is portable.
//!
//! ## What this module does (project context)
//! The project goal is to map where a Rust program spends its executions
//! (count-heat), later its time (time-heat), and then combine them
//! (meta-heat quadrant), to find the "low staves" of the Liebig barrel:
//! the low-hanging fruit.
//!
//! POC-1 does count-heat only:
//! 1. Read an LCOV file produced by `cargo llvm-cov` from an instrumented
//!    (`-C instrument-coverage`) run of the target program.
//! 2. FUNCTION sites: from `FN:`/`FNDA:` records. Function entry counts are
//!    exact (LLVM counters, not samples). Monomorphized instances that share
//!    one source line are summed into one site, and the number of merged
//!    instances is reported.
//! 3. LOOP sites: a lexical line scan of each non-cold source file finds loop
//!    headers (`for`, `while`, `loop`, optionally labeled). The loop's count is
//!    the LCOV `DA:` execution count of the header line, which approximates
//!    the iteration count. Each loop is attributed to the nearest preceding
//!    function definition line in that file.
//! 4. Rank (hottest first, deterministic tie-break by file then line), then
//!    summarize: totals, zero-count sites, concentration (how many sites carry
//!    50% / 90% / 99% of counts), cold tail, order-of-magnitude histogram.
//! 5. Write a text report (file + stdout) and a TSV with every site
//!    (machine-readable input for POC-2, which will join time-heat on
//!    `source_path:line`).
//!
//! ## Known limitations (documented features, not bugs, for POC-1)
//! - Loop detection is lexical: loops that do not start a line
//!   (`let x = loop {`, `} else { for ...`) are missed; a line inside a
//!   multi-line string that starts with `for ` may be a false positive (it
//!   usually has no line count and is then reported as "no line data").
//!   Block comments are tracked only when `/*` starts a line.
//! - Iterator-adaptor loops (`.for_each(...)`, `.map(...)`) are not loop sites,
//!   but their closures appear as function sites (LLVM instruments closures).
//! - Stop condition is "D": whenever the instrumented program exited cleanly.
//!   No in-tool duration or u32 cap in POC-1.
//! - Function names may be mangled depending on tool versions; identity is
//!   `source_path:line`, so counts are unaffected.
//! - TSV fields are not escaped (paths containing tabs would break columns).
//! - Over-long source lines (since POC-1.2): a source line longer than
//!   `MAX_LINE_BYTES` (64 KiB) no longer causes the whole file to be skipped.
//!   The first 64 KiB of the line are scanned exactly like a normal line (a
//!   loop keyword or comment marker at the start of the line is still found).
//!   The rest of the line is read and discarded; while discarding, the scanner
//!   checks only for a block-comment end `*/`, so comment tracking stays
//!   correct. The file is skipped only if the discarded remainder of one line
//!   exceeds `MAX_SKIPPED_LINE_BYTES` (64 MiB). Over-long LCOV lines remain a
//!   fatal error, because a real LCOV line is always short.
//! - Block-comment tracking uses "does this line contain `*/`": a line that
//!   closes one block comment and opens another (`*/ ... /*`) is treated as
//!   closed. This applies to normal and over-long lines alike.
//!
//! ## Modes
//! - Production-release: never panics; errors are 2-byte `HeatmapError` codes;
//!   no printing of errors inside functions (only the top level reports a code).
//! - Debug: verbose `eprintln!` diagnostics (gated `#[cfg(debug_assertions)]`);
//!   internal-invariant `debug_assert!` gated
//!   `#[cfg(all(debug_assertions, not(test)))]`.
//! - Test: `#[cfg(test)]` unit tests using `assert!`/`unwrap`.
//!
//! ## Recovery tiers
//! - Tier 1: transient file-open errors retried with bounded backoff.
//! - Tier 2: a source file that cannot be scanned is skipped and counted
//!   (reported in the provenance block); the analysis continues.
//! - Tier 3: top-level (`main`) logs the numeric code and exits non-zero.
//!
//! ## Heap policy
//! Heap is used for analysis data (unavoidable for variable-size input),
//! capped by the `MAX_*` constants. The error path never allocates.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
// ============================================================================
// Constants (all loops and buffers are bounded by these)
// ============================================================================

/// Maximum number of `--flag value` pairs accepted on the command line.
const MAX_CLI_FLAG_PAIRS: usize = 8;

/// Default number of rows in each Top-N table when `--top-n` is not given.
const DEFAULT_TOP_N: u16 = 20;

/// Upper bound for `--top-n` (keeps the text report human-sized).
const MAX_TOP_N: u16 = 10_000;

/// Maximum bytes kept for one line of text (the newline byte counts too).
///
/// - LCOV input: a longer line is a fatal error (`LcovLineTooLong`), because
///   every real LCOV record line is short; a longer one means corrupt input.
/// - Source files (loop scan, since POC-1.2): a longer line is NOT an error.
///   Its first `MAX_LINE_BYTES + 1` bytes are kept and scanned normally (loop
///   keywords and comment markers sit at the start of a line), and the rest
///   of the line is read and discarded. See `read_source_line_skipping_overlong`.
const MAX_LINE_BYTES: usize = 64 * 1024;

/// Source scan only: the longest remainder of one over-long line that the
/// scanner will read and discard before giving up on that file.
///
/// Why a cap exists: discarding is a loop, and every loop in this tool is
/// bounded. 64 MiB is far above any line expected even in generated source
/// files. If one line's remainder exceeds it, the file is skipped (Tier 2)
/// and counted under "files skipped: line > MAX_SKIPPED_LINE_BYTES".
const MAX_SKIPPED_LINE_BYTES: usize = 64 * 1024 * 1024;

/// Source scan only: extra read calls allowed while discarding, on top of
/// one call per discarded byte.
///
/// Every successful read call discards at least one byte, so the byte cap
/// already bounds them. Only "interrupted" reads (the operating system asks
/// the program to try again; nothing is read) use these extra calls. This
/// keeps the discard loop finite even if a reader reports "interrupted" forever.
const SKIP_LINE_EXTRA_READ_CALLS: usize = 1024;

/// Maximum number of lines read from one LCOV file.
const MAX_LCOV_LINES: usize = 50_000_000;

/// Maximum number of `FN`/`FNDA`/`DA` entries accepted from one LCOV file.
const MAX_COVERAGE_ENTRIES: usize = 20_000_000;

/// Maximum number of source-file records (`SF:`) accepted from one LCOV file.
const MAX_SOURCE_FILES: usize = 200_000;

/// Maximum number of lines scanned in one source file.
const MAX_SOURCE_LINES_PER_FILE: usize = 1_000_000;

/// Tier-1 retry: maximum attempts to open a file on transient errors.
const MAX_OPEN_ATTEMPTS: u32 = 3;

/// Tier-1 retry: backoff step in milliseconds (attempt n sleeps n * step).
const OPEN_RETRY_BACKOFF_STEP_MS: u64 = 50;

/// Concentration targets: "the hottest K sites carry P% of all counts".
const CONCENTRATION_TARGETS_PERCENT: [u128; 3] = [50, 90, 99];

/// Order-of-magnitude histogram buckets: bucket 0 = count 0,
/// bucket d (1..=20) = count in [10^(d-1), 10^d). u64::MAX has 20 digits.
const DECADE_BUCKET_COUNT: usize = 21;

/// Initial capacity for in-memory report buffers.
const REPORT_BUFFER_INITIAL_CAPACITY: usize = 64 * 1024;

/// Output file name for the human-readable report (inside `--out-dir`).
const REPORT_FILE_NAME: &str = "hix_report.txt";

/// Output file name for the full per-site TSV data (inside `--out-dir`).
const SITES_TSV_FILE_NAME: &str = "hix_sites.tsv";

/// Error log file name, created next to the executable (codes only, no PII).
const ERROR_LOG_FILE_NAME: &str = "hix_error_log.txt";

/// Usage text shown by `main` for command-line errors (static: no heap).
pub const USAGE_TEXT: &str = "\
usage: hix --lcov /ABSOLUTE/path/file.lcov --out-dir /ABSOLUTE/output/dir [--top-n 1..10000]

  --lcov     LCOV file from: cargo llvm-cov report --lcov --output-path /ABSOLUTE/file.lcov
  --out-dir  directory for hix_report.txt and hix_sites.tsv
  --top-n    rows per Top-N table (default 20)
All paths must be absolute.";

/// Minimum count for a site to take part in co-count groups. Identical counts
/// below this are too likely to be coincidence to indicate structural coupling.
const CO_COUNT_MIN_COUNT: u64 = 100_000;

// ============================================================================
// Error codes
// ============================================================================

/// Project-wide fieldless error code (2 bytes, `Copy`, no heap, no PII).
///
/// Codes are append-only: never renumber or reuse.
///
/// | Block   | Area                                            |
/// |---------|-------------------------------------------------|
/// | 100–149 | command-line arguments                          |
/// | 150–199 | enforced custom types (value integrity)         |
/// | 200–299 | LCOV input file                                 |
/// | 300–399 | source-file loop scan (absorbed at Tier 2)      |
/// | 400–499 | analysis / aggregation                          |
/// | 500–599 | report output                                   |
/// | 600–699 | error log                                       |
// `Debug` (variant names as text) exists only in test and debug builds, so a
// production-release binary contains no variant-name strings, and any stray
// `{:?}` on a HeatmapError in production-release code fails to compile.
// This is an intended guard (see the Modes & Case Handling framework).
#[cfg_attr(any(test, debug_assertions), derive(Debug))]
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum HeatmapError {
    // 100–149: command line
    CliTooManyArguments = 101,
    CliUnknownFlag = 102,
    CliMissingFlagValue = 103,
    CliMissingLcovPath = 104,
    CliMissingOutputDirectory = 105,
    CliTopNNotUtf8 = 106,
    CliTopNNotANumber = 107,
    CliDuplicateFlag = 108,

    // 150–199: enforced custom types
    TypeTopNOutOfRange = 150,
    TypeTopNCorrupted = 151,
    TypeLineNumberZero = 152,
    TypeLineNumberCorrupted = 153,
    TypePathNotAbsolute = 154,
    TypePathCorrupted = 155,

    // 200–299: LCOV input
    LcovOpenNotFound = 201,
    LcovOpenPermissionDenied = 202,
    LcovOpenTransient = 203,
    LcovOpenOther = 204,
    LcovReadFailed = 205,
    LcovLineTooLong = 206,
    LcovTooManyLines = 207,
    LcovNotUtf8 = 208,
    LcovRecordOutsideFile = 209,
    LcovBadLineNumber = 210,
    LcovBadCount = 211,
    LcovMalformedRecord = 212,
    LcovTooManyEntries = 213,
    LcovUnterminatedRecord = 214,
    LcovNoData = 215,
    LcovCountOverflow = 216,
    LcovTooManyFiles = 217,

    // 300–399: source scan (Tier 2: absorbed per file, counted in report)
    SrcOpenNotFound = 301,
    SrcOpenPermissionDenied = 302,
    SrcOpenTransient = 303,
    SrcOpenOther = 304,
    SrcReadFailed = 305,
    SrcTooManyLines = 306,
    SrcLineNumberOverflow = 307,
    SrcLineExceedsSkipCap = 308,
    SrcSkipReadFailed = 309,
    SrcSkipReadCallLimit = 310,

    // 400–499: analysis
    AnaCountOverflow = 401,
    AnaFileIndexInvalid = 402,
    AnaSiteIndexInvalid = 403,
    AnaHistogramIndexInvalid = 404,
    AnaFunctionLineIndexOverflow = 405,
    AnaCoCountRunInvalid = 406,

    // 500–599: report output
    RptCreateDirectoryFailed = 501,
    RptOpenNotFound = 502,
    RptOpenPermissionDenied = 503,
    RptOpenTransient = 504,
    RptOpenOther = 505,
    RptBufferWriteFailed = 506,
    RptFileWriteFailed = 507,
    RptFlushFailed = 508,
    RptStdoutWriteFailed = 509,

    // 600–699: error log
    LogExecutablePathUnavailable = 601,
    LogExecutableParentMissing = 602,
    LogOpenFailed = 603,
    LogWriteFailed = 604,
    LogClockBeforeEpoch = 605,
}

impl HeatmapError {
    /// Numeric code (stable, append-only). Safe to print or log in any mode.
    pub fn code(self) -> u16 {
        self as u16
    }

    /// Tier-1 policy: may the caller retry the same bounded operation?
    /// Defined per code (exhaustive, no wildcard) so new codes force a decision.
    pub fn is_retryable(self) -> bool {
        match self {
            HeatmapError::LcovOpenTransient
            | HeatmapError::SrcOpenTransient
            | HeatmapError::RptOpenTransient => true,

            HeatmapError::AnaFunctionLineIndexOverflow
            | HeatmapError::AnaCoCountRunInvalid
            | HeatmapError::CliTooManyArguments
            | HeatmapError::CliUnknownFlag
            | HeatmapError::CliMissingFlagValue
            | HeatmapError::CliMissingLcovPath
            | HeatmapError::CliMissingOutputDirectory
            | HeatmapError::CliTopNNotUtf8
            | HeatmapError::CliTopNNotANumber
            | HeatmapError::CliDuplicateFlag
            | HeatmapError::TypeTopNOutOfRange
            | HeatmapError::TypeTopNCorrupted
            | HeatmapError::TypeLineNumberZero
            | HeatmapError::TypeLineNumberCorrupted
            | HeatmapError::TypePathNotAbsolute
            | HeatmapError::TypePathCorrupted
            | HeatmapError::LcovOpenNotFound
            | HeatmapError::LcovOpenPermissionDenied
            | HeatmapError::LcovOpenOther
            | HeatmapError::LcovReadFailed
            | HeatmapError::LcovLineTooLong
            | HeatmapError::LcovTooManyLines
            | HeatmapError::LcovNotUtf8
            | HeatmapError::LcovRecordOutsideFile
            | HeatmapError::LcovBadLineNumber
            | HeatmapError::LcovBadCount
            | HeatmapError::LcovMalformedRecord
            | HeatmapError::LcovTooManyEntries
            | HeatmapError::LcovUnterminatedRecord
            | HeatmapError::LcovNoData
            | HeatmapError::LcovCountOverflow
            | HeatmapError::LcovTooManyFiles
            | HeatmapError::SrcOpenNotFound
            | HeatmapError::SrcOpenPermissionDenied
            | HeatmapError::SrcOpenOther
            | HeatmapError::SrcReadFailed
            | HeatmapError::SrcTooManyLines
            | HeatmapError::SrcLineNumberOverflow
            | HeatmapError::SrcLineExceedsSkipCap
            | HeatmapError::SrcSkipReadFailed
            | HeatmapError::SrcSkipReadCallLimit
            | HeatmapError::AnaCountOverflow
            | HeatmapError::AnaFileIndexInvalid
            | HeatmapError::AnaSiteIndexInvalid
            | HeatmapError::AnaHistogramIndexInvalid
            | HeatmapError::RptCreateDirectoryFailed
            | HeatmapError::RptOpenNotFound
            | HeatmapError::RptOpenPermissionDenied
            | HeatmapError::RptOpenOther
            | HeatmapError::RptBufferWriteFailed
            | HeatmapError::RptFileWriteFailed
            | HeatmapError::RptFlushFailed
            | HeatmapError::RptStdoutWriteFailed
            | HeatmapError::LogExecutablePathUnavailable
            | HeatmapError::LogExecutableParentMissing
            | HeatmapError::LogOpenFailed
            | HeatmapError::LogWriteFailed
            | HeatmapError::LogClockBeforeEpoch => false,
        }
    }

    /// True for errors the user fixes by changing the command line
    /// (used only to decide whether `main` shows `USAGE_TEXT`).
    pub fn is_cli_usage_error(self) -> bool {
        matches!(
            self,
            HeatmapError::CliTooManyArguments
                | HeatmapError::CliUnknownFlag
                | HeatmapError::CliMissingFlagValue
                | HeatmapError::CliMissingLcovPath
                | HeatmapError::CliMissingOutputDirectory
                | HeatmapError::CliTopNNotUtf8
                | HeatmapError::CliTopNNotANumber
                | HeatmapError::CliDuplicateFlag
                | HeatmapError::TypeTopNOutOfRange
                | HeatmapError::TypePathNotAbsolute
        )
    }
}

/// Human-readable descriptions: compiled only in debug and test builds.
#[cfg(any(debug_assertions, test))]
impl std::fmt::Display for HeatmapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description: &str = match self {
            HeatmapError::AnaFunctionLineIndexOverflow => {
                "function-line index count sum overflowed"
            }
            HeatmapError::AnaCoCountRunInvalid => "co-count group range invalid",
            HeatmapError::CliTooManyArguments => "too many command-line arguments",
            HeatmapError::CliUnknownFlag => "unknown command-line flag",
            HeatmapError::CliMissingFlagValue => "flag given without a value",
            HeatmapError::CliMissingLcovPath => "--lcov is required",
            HeatmapError::CliMissingOutputDirectory => "--out-dir is required",
            HeatmapError::CliTopNNotUtf8 => "--top-n value is not UTF-8",
            HeatmapError::CliTopNNotANumber => "--top-n value is not a u16 number",
            HeatmapError::CliDuplicateFlag => "a flag was given more than once",
            HeatmapError::TypeTopNOutOfRange => "top-n outside 1..=MAX_TOP_N",
            HeatmapError::TypeTopNCorrupted => "TopNCount failed validity recheck",
            HeatmapError::TypeLineNumberZero => "source line number must be >= 1",
            HeatmapError::TypeLineNumberCorrupted => "SourceLineNumber failed validity recheck",
            HeatmapError::TypePathNotAbsolute => "path must be absolute",
            HeatmapError::TypePathCorrupted => "AbsolutePath failed validity recheck",
            HeatmapError::LcovOpenNotFound => "lcov file not found",
            HeatmapError::LcovOpenPermissionDenied => "lcov file permission denied",
            HeatmapError::LcovOpenTransient => {
                "lcov file open: transient error (retries exhausted)"
            }
            HeatmapError::LcovOpenOther => "lcov file open failed",
            HeatmapError::LcovReadFailed => "lcov read failed",
            HeatmapError::LcovLineTooLong => "lcov line exceeds MAX_LINE_BYTES",
            HeatmapError::LcovTooManyLines => "lcov file exceeds MAX_LCOV_LINES",
            HeatmapError::LcovNotUtf8 => "lcov line is not UTF-8",
            HeatmapError::LcovRecordOutsideFile => "lcov entry outside an SF..end_of_record block",
            HeatmapError::LcovBadLineNumber => "lcov line number invalid",
            HeatmapError::LcovBadCount => "lcov execution count invalid",
            HeatmapError::LcovMalformedRecord => "lcov record malformed",
            HeatmapError::LcovTooManyEntries => "lcov exceeds MAX_COVERAGE_ENTRIES",
            HeatmapError::LcovUnterminatedRecord => "lcov ended inside a record",
            HeatmapError::LcovNoData => "lcov contains no source-file records",
            HeatmapError::LcovCountOverflow => "lcov count sum overflowed u64",
            HeatmapError::LcovTooManyFiles => "lcov exceeds MAX_SOURCE_FILES",
            HeatmapError::SrcOpenNotFound => "source file not found",
            HeatmapError::SrcOpenPermissionDenied => "source file permission denied",
            HeatmapError::SrcOpenTransient => {
                "source file open: transient error (retries exhausted)"
            }
            HeatmapError::SrcOpenOther => "source file open failed",
            HeatmapError::SrcReadFailed => "source file read failed",
            HeatmapError::SrcTooManyLines => "source file exceeds MAX_SOURCE_LINES_PER_FILE",
            HeatmapError::SrcLineNumberOverflow => "source line number overflowed u32",
            HeatmapError::SrcLineExceedsSkipCap => {
                "over-long source line remainder exceeds MAX_SKIPPED_LINE_BYTES; file skipped"
            }
            HeatmapError::SrcSkipReadFailed => {
                "source read failed while discarding the rest of an over-long line"
            }
            HeatmapError::SrcSkipReadCallLimit => {
                "discarding an over-long source line exceeded its read-call limit"
            }
            HeatmapError::AnaCountOverflow => "count total overflowed",
            HeatmapError::AnaFileIndexInvalid => "site refers to a missing file record",
            HeatmapError::AnaSiteIndexInvalid => "site index invalid during merge",
            HeatmapError::AnaHistogramIndexInvalid => "histogram bucket index invalid",
            HeatmapError::RptCreateDirectoryFailed => "could not create output directory",
            HeatmapError::RptOpenNotFound => "report path not found",
            HeatmapError::RptOpenPermissionDenied => "report path permission denied",
            HeatmapError::RptOpenTransient => "report open: transient error (retries exhausted)",
            HeatmapError::RptOpenOther => "report open failed",
            HeatmapError::RptBufferWriteFailed => "writing report into memory buffer failed",
            HeatmapError::RptFileWriteFailed => "writing report file failed",
            HeatmapError::RptFlushFailed => "flushing report file failed",
            HeatmapError::RptStdoutWriteFailed => "writing report to stdout failed",
            HeatmapError::LogExecutablePathUnavailable => "executable path unavailable",
            HeatmapError::LogExecutableParentMissing => "executable has no parent directory",
            HeatmapError::LogOpenFailed => "error log open failed",
            HeatmapError::LogWriteFailed => "error log write failed",
            HeatmapError::LogClockBeforeEpoch => "system clock is before UNIX epoch",
        };
        write!(formatter, "CHM-{}: {}", self.code(), description)
    }
}

// ============================================================================
// Enforced custom types (value integrity)
// ============================================================================

/// Number of rows per Top-N table. Valid range: 1..=MAX_TOP_N.
/// Field is private: construct only through `new`, read only through `get`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopNCount {
    top_n_value: u16,
}

impl TopNCount {
    /// Validates and constructs. Input-validation error (user-facing).
    pub fn new(candidate_top_n: u16) -> Result<Self, HeatmapError> {
        if candidate_top_n == 0 || candidate_top_n > MAX_TOP_N {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: top-n {} outside 1..={}",
                HeatmapError::TypeTopNOutOfRange.code(),
                candidate_top_n,
                MAX_TOP_N
            );
            return Err(HeatmapError::TypeTopNOutOfRange);
        }
        Ok(Self {
            top_n_value: candidate_top_n,
        })
    }

    /// Returns the value after re-checking validity (guards corruption).
    pub fn get(&self) -> Result<u16, HeatmapError> {
        if self.top_n_value == 0 || self.top_n_value > MAX_TOP_N {
            #[cfg(all(debug_assertions, not(test)))]
            debug_assert!(
                false,
                "CHM-{}: TopNCount corrupted: {}",
                HeatmapError::TypeTopNCorrupted.code(),
                self.top_n_value
            );
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: TopNCount recheck failed: {}",
                HeatmapError::TypeTopNCorrupted.code(),
                self.top_n_value
            );
            return Err(HeatmapError::TypeTopNCorrupted);
        }
        Ok(self.top_n_value)
    }
}

/// 1-based source line number (LCOV and editors are 1-based; 0 is invalid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceLineNumber {
    line_value: u32,
}

impl SourceLineNumber {
    /// Validates and constructs (line >= 1).
    pub fn new(candidate_line: u32) -> Result<Self, HeatmapError> {
        if candidate_line == 0 {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: line number 0 is invalid",
                HeatmapError::TypeLineNumberZero.code()
            );
            return Err(HeatmapError::TypeLineNumberZero);
        }
        Ok(Self {
            line_value: candidate_line,
        })
    }

    /// Returns the value after re-checking validity.
    pub fn get(&self) -> Result<u32, HeatmapError> {
        if self.line_value == 0 {
            #[cfg(all(debug_assertions, not(test)))]
            debug_assert!(
                false,
                "CHM-{}: SourceLineNumber corrupted: 0",
                HeatmapError::TypeLineNumberCorrupted.code()
            );
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: SourceLineNumber recheck failed",
                HeatmapError::TypeLineNumberCorrupted.code()
            );
            return Err(HeatmapError::TypeLineNumberCorrupted);
        }
        Ok(self.line_value)
    }
}

/// A path guaranteed (and re-checked) to be absolute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsolutePath {
    absolute_path: PathBuf,
}

impl AbsolutePath {
    /// Validates and constructs (path must be absolute).
    pub fn new(candidate_path: PathBuf) -> Result<Self, HeatmapError> {
        if !candidate_path.is_absolute() {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: not absolute: {}",
                HeatmapError::TypePathNotAbsolute.code(),
                candidate_path.display()
            );
            return Err(HeatmapError::TypePathNotAbsolute);
        }
        Ok(Self {
            absolute_path: candidate_path,
        })
    }

    /// Returns the path after re-checking validity.
    pub fn get(&self) -> Result<&Path, HeatmapError> {
        if !self.absolute_path.is_absolute() {
            #[cfg(all(debug_assertions, not(test)))]
            debug_assert!(
                false,
                "CHM-{}: AbsolutePath corrupted",
                HeatmapError::TypePathCorrupted.code()
            );
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: AbsolutePath recheck failed",
                HeatmapError::TypePathCorrupted.code()
            );
            return Err(HeatmapError::TypePathCorrupted);
        }
        Ok(self.absolute_path.as_path())
    }
}

/// Validated run configuration produced from the command line.
pub struct HeatmapRunConfiguration {
    lcov_input_path: AbsolutePath,
    output_directory_path: AbsolutePath,
    top_n: TopNCount,
}

// ============================================================================
// Internal data model
// ============================================================================

/// Coverage data for one `SF:` ... `end_of_record` block (one source file).
#[derive(Debug)]
struct FileCoverageRecord {
    /// Source path exactly as written in LCOV (normally absolute with cargo llvm-cov).
    source_path_text: String,
    /// `FN:<line>,<name>` entries, in file order.
    function_definitions: Vec<(SourceLineNumber, String)>,
    /// `FNDA:<count>,<name>` entries, summed per name.
    function_entry_counts: HashMap<String, u64>,
    /// `DA:<line>,<count>` entries (max kept if a line repeats).
    line_execution_counts: HashMap<SourceLineNumber, u64>,
    /// Max of all `DA` counts in this file: 0 means the whole file is cold.
    max_line_execution_count: u64,
}

impl FileCoverageRecord {
    fn new(source_path_text: &str) -> Self {
        Self {
            source_path_text: source_path_text.to_string(),
            function_definitions: Vec::new(),
            function_entry_counts: HashMap::new(),
            line_execution_counts: HashMap::new(),
            max_line_execution_count: 0,
        }
    }
}

/// One ranked heat site (a function or a loop header) identified by file + line.
#[derive(Debug, Clone)]
struct HeatSite {
    /// Index into the `FileCoverageRecord` slice (resolves to the source path).
    source_file_index: usize,
    /// 1-based line of the function definition or loop header.
    line_number: SourceLineNumber,
    /// Function name (possibly mangled) or "for loop in <function>".
    display_name: String,
    /// Function: entry count (calls). Loop: header-line count (≈ iterations).
    execution_count: u64,
    /// Number of LCOV function instances merged into this site (generics).
    merged_instance_count: u32,
    /// Loops only: summed entry count (all instances) of the enclosing
    /// function's definition line. `None` for function sites, or when no
    /// enclosing function was found. Basis of the derived ratio
    /// "loop header executions per call of the enclosing function".
    enclosing_function_entry_count: Option<u64>,
}

/// Which loop keyword began a detected loop header line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopKeyword {
    For,
    While,
    Loop,
}

impl LoopKeyword {
    fn as_label(self) -> &'static str {
        match self {
            LoopKeyword::For => "for",
            LoopKeyword::While => "while",
            LoopKeyword::Loop => "loop",
        }
    }
}

/// Transparency counters for the loop scan (reported in the
/// `[loop-scan coverage]` block of the text report).
///
/// Every source file listed in the LCOV is counted exactly once: either as
/// scanned, or under exactly one skip reason. No file is silently unexplained.
#[derive(Debug, Default, Clone, Copy)]
struct LoopScanStatistics {
    /// Source files listed in the LCOV.
    files_total: usize,
    /// Files read to the end and searched for loop headers.
    files_scanned: usize,
    /// Files skipped because every line count is 0 (cannot contain a hot loop).
    files_skipped_cold: usize,
    /// Files skipped because the LCOV path is not absolute.
    files_skipped_non_absolute_path: usize,
    /// Files skipped because they no longer exist at the LCOV path.
    files_skipped_not_found: usize,
    /// Files skipped because they could not be opened (permissions).
    files_skipped_permission_denied: usize,
    /// Files abandoned because one line's discarded remainder exceeded
    /// `MAX_SKIPPED_LINE_BYTES`. Replaces POC-1.1's `files_skipped_line_too_long`,
    /// which skipped a whole file for any line over `MAX_LINE_BYTES`.
    files_skipped_line_exceeds_skip_cap: usize,
    /// Files abandoned because they exceed `MAX_SOURCE_LINES_PER_FILE`.
    files_skipped_too_many_lines: usize,
    /// Files abandoned for any other reason (read failures and similar).
    files_skipped_other_error: usize,
    /// Scanned files that contain at least one over-long line.
    files_with_overlong_source_lines: usize,
    /// Over-long lines across all scanned files (start scanned, rest discarded).
    overlong_source_lines_total: usize,
    /// Loop headers found by the lexical scan, with or without a line count.
    loop_headers_found: usize,
    /// Loop headers whose line has no LCOV `DA` count (not instrumented,
    /// compiled out, or a lexical false positive).
    loop_headers_without_line_data: usize,
}

/// Result of scanning one source file.
#[derive(Debug, Default)]
struct SourceScanOutcome {
    /// Loop sites found in this file (only headers with a line count).
    loop_sites: Vec<HeatSite>,
    /// Loop headers found, with or without a line count.
    loop_headers_found: usize,
    /// Loop headers whose line has no LCOV `DA` count.
    loop_headers_without_line_data: usize,
    /// Lines longer than `MAX_LINE_BYTES` in this file (start scanned, rest discarded).
    overlong_lines_found: usize,
}

/// What `read_source_line_skipping_overlong` found when reading one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceLineReadOutcome {
    /// No more lines: the file has been read to the end.
    EndOfInput,
    /// A whole line (at most `MAX_LINE_BYTES` bytes) is in the line buffer.
    CompleteLine,
    /// The line was longer than `MAX_LINE_BYTES`. The line buffer holds its
    /// first `MAX_LINE_BYTES + 1` bytes; the rest has been read and discarded.
    OverLongLineStartKept {
        /// True if a block-comment end `*/` occurred in the discarded part,
        /// including a `*/` split between the kept start and the discarded part.
        discarded_part_contains_block_comment_end: bool,
    },
}

/// Summary statistics for one kind of site (functions or loops).
#[derive(Debug, Clone, Copy)]
struct HeatKindSummary {
    site_count: usize,
    zero_count_sites: usize,
    total_count: u128,
    /// Aligned with `CONCENTRATION_TARGETS_PERCENT`; 0 means "n/a" (total = 0).
    sites_reaching_percent: [usize; 3],
    decade_histogram: [u64; DECADE_BUCKET_COUNT],
}

/// Which way to open a file.
#[derive(Debug, Clone, Copy)]
enum FileOpenPurpose {
    ReadExisting,
    CreateTruncate,
}

/// Per-call-site error codes for file opening, so each site keeps unique codes.
///
/// No `Debug` derive: the fields are `HeatmapError` values, which have no
/// `Debug` in production-release builds (no variant-name strings ship).
#[derive(Clone, Copy)]
struct OpenErrorCodes {
    not_found: HeatmapError,
    permission_denied: HeatmapError,
    transient: HeatmapError,
    other: HeatmapError,
}

const LCOV_OPEN_ERROR_CODES: OpenErrorCodes = OpenErrorCodes {
    not_found: HeatmapError::LcovOpenNotFound,
    permission_denied: HeatmapError::LcovOpenPermissionDenied,
    transient: HeatmapError::LcovOpenTransient,
    other: HeatmapError::LcovOpenOther,
};

const SOURCE_OPEN_ERROR_CODES: OpenErrorCodes = OpenErrorCodes {
    not_found: HeatmapError::SrcOpenNotFound,
    permission_denied: HeatmapError::SrcOpenPermissionDenied,
    transient: HeatmapError::SrcOpenTransient,
    other: HeatmapError::SrcOpenOther,
};

const REPORT_OPEN_ERROR_CODES: OpenErrorCodes = OpenErrorCodes {
    not_found: HeatmapError::RptOpenNotFound,
    permission_denied: HeatmapError::RptOpenPermissionDenied,
    transient: HeatmapError::RptOpenTransient,
    other: HeatmapError::RptOpenOther,
};

/// Everything the report writers need (borrowed; no copies).
struct ReportInputs<'a> {
    lcov_input_path: &'a Path,
    report_file_path: &'a Path,
    sites_tsv_file_path: &'a Path,
    records: &'a [FileCoverageRecord],
    function_sites: &'a [HeatSite],
    function_summary: &'a HeatKindSummary,
    loop_sites: &'a [HeatSite],
    loop_summary: &'a HeatKindSummary,
    loop_scan_statistics: &'a LoopScanStatistics,
    top_n_limit: usize,
    /// Duplicate `FN` names within a file (integrity check; expected 0).
    duplicate_function_name_count: usize,
}

// ============================================================================
// Public entry points
// ============================================================================

/// Runs POC-1 end to end using the process command line.
///
/// Steps: parse args -> parse LCOV -> function sites -> loop sites ->
/// summarize -> write text report + TSV -> print report to stdout.
pub fn run_heatmap_cli() -> Result<(), HeatmapError> {
    let run_configuration = parse_cli_arguments(std::env::args_os())?;
    run_heatmap_with_configuration(&run_configuration)
}

/// Appends `unix_seconds=<n> error_code=<code>` to the error log next to the
/// executable. Logs only the numeric code (no paths, no user data).
pub fn append_error_code_to_log(error_code: HeatmapError) -> Result<(), HeatmapError> {
    let executable_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(_io_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: current_exe failed: {}",
                HeatmapError::LogExecutablePathUnavailable.code(),
                _io_error
            );
            return Err(HeatmapError::LogExecutablePathUnavailable);
        }
    };
    let executable_directory = match executable_path.parent() {
        Some(directory) => directory,
        None => return Err(HeatmapError::LogExecutableParentMissing),
    };
    let error_log_path = executable_directory.join(ERROR_LOG_FILE_NAME);

    let unix_seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_clock_error) => return Err(HeatmapError::LogClockBeforeEpoch),
    };

    let mut error_log_file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&error_log_path)
    {
        Ok(file) => file,
        Err(_io_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: error log open failed: {}",
                HeatmapError::LogOpenFailed.code(),
                _io_error
            );
            return Err(HeatmapError::LogOpenFailed);
        }
    };

    match writeln!(
        error_log_file,
        "unix_seconds={} error_code={}",
        unix_seconds,
        error_code.code()
    ) {
        Ok(()) => Ok(()),
        Err(_io_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: error log write failed: {}",
                HeatmapError::LogWriteFailed.code(),
                _io_error
            );
            Err(HeatmapError::LogWriteFailed)
        }
    }
}

// ============================================================================
// Orchestration
// ============================================================================

/// Runs the full POC-1.1 count-heat pipeline for an already-validated configuration.
///
/// # Project context
/// This is the single orchestration point of the tool. Each step consumes the
/// previous step's output; nothing is written to disk until every report has
/// been rendered successfully in memory, so a failure never leaves a
/// half-written report next to a complete one.
///
/// # Steps
/// 1. Open and parse the LCOV file (fatal on failure: no LCOV, no data).
/// 2. Build function sites (exact LLVM entry counts; monomorphized instances
///    at one source line merged), then sort + summarize them.
/// 3. Scan non-cold source files for loop headers (Tier 2: an unreadable file
///    is skipped and counted, never fatal), then sort + summarize loop sites.
/// 4. Render the text report and the all-sites TSV into memory.
/// 5. Create the output directory, write both files, then print the text
///    report to stdout.
///
/// # Arguments
/// - `run_configuration`: validated paths (absolute) and Top-N limit.
///   Each value is re-checked through its `.get()` before use.
///
/// # Returns
/// - `Ok(())` when both files were written and the report was printed.
/// - `Err(HeatmapError)`: the unique code of the first failing step,
///   propagated unchanged to `main` (Tier 3), which logs the numeric code.
///
/// # Modes
/// - Production: no printing here; errors are returned as codes only.
/// - Debug: callees print verbose diagnostics via gated `eprintln!`.
fn run_heatmap_with_configuration(
    run_configuration: &HeatmapRunConfiguration,
) -> Result<(), HeatmapError> {
    let lcov_input_path = run_configuration.lcov_input_path.get()?;
    let output_directory_path = run_configuration.output_directory_path.get()?;
    let top_n_limit = usize::from(run_configuration.top_n.get()?);

    // Step 1: parse LCOV (fatal on failure: without it there is no data).
    let lcov_file = open_file_with_retry(
        lcov_input_path,
        FileOpenPurpose::ReadExisting,
        LCOV_OPEN_ERROR_CODES,
    )?;
    let mut lcov_reader = BufReader::new(lcov_file);
    let coverage_records = parse_lcov_reader(&mut lcov_reader)?;

    // Step 2: function sites (exact entry counts), sorted + summarized together.
    let (mut function_sites, duplicate_function_name_count) =
        build_function_sites(&coverage_records)?;
    let function_summary = sort_and_summarize_sites(&mut function_sites)?;

    // Step 3: loop sites (lexical scan; per-file failures absorbed at Tier 2),
    // sorted + summarized together.
    let (mut loop_sites, loop_scan_statistics) = scan_all_files_for_loops(&coverage_records);
    let loop_summary = sort_and_summarize_sites(&mut loop_sites)?;

    // Step 4: render reports into memory (no disk writes yet).
    let report_file_path = output_directory_path.join(REPORT_FILE_NAME);
    let sites_tsv_file_path = output_directory_path.join(SITES_TSV_FILE_NAME);
    let report_inputs = ReportInputs {
        lcov_input_path,
        report_file_path: &report_file_path,
        sites_tsv_file_path: &sites_tsv_file_path,
        records: &coverage_records,
        function_sites: &function_sites,
        function_summary: &function_summary,
        loop_sites: &loop_sites,
        loop_summary: &loop_summary,
        loop_scan_statistics: &loop_scan_statistics,
        top_n_limit,
        duplicate_function_name_count,
    };

    let mut report_bytes: Vec<u8> = Vec::with_capacity(REPORT_BUFFER_INITIAL_CAPACITY);
    write_text_report(&mut report_bytes, &report_inputs)?;
    let mut sites_tsv_bytes: Vec<u8> = Vec::with_capacity(REPORT_BUFFER_INITIAL_CAPACITY);
    write_sites_tsv(&mut sites_tsv_bytes, &report_inputs)?;

    // Step 5: write files, then stdout.
    if let Err(_io_error) = fs::create_dir_all(output_directory_path) {
        #[cfg(debug_assertions)]
        eprintln!(
            "CHM-{}: create_dir_all failed: {}",
            HeatmapError::RptCreateDirectoryFailed.code(),
            _io_error
        );
        return Err(HeatmapError::RptCreateDirectoryFailed);
    }
    write_bytes_to_new_file(&report_file_path, &report_bytes)?;
    write_bytes_to_new_file(&sites_tsv_file_path, &sites_tsv_bytes)?;

    let standard_output = std::io::stdout();
    let mut standard_output_lock = standard_output.lock();
    if let Err(_io_error) = standard_output_lock.write_all(&report_bytes) {
        #[cfg(debug_assertions)]
        eprintln!(
            "CHM-{}: stdout write failed: {}",
            HeatmapError::RptStdoutWriteFailed.code(),
            _io_error
        );
        return Err(HeatmapError::RptStdoutWriteFailed);
    }
    if let Err(_io_error) = standard_output_lock.flush() {
        #[cfg(debug_assertions)]
        eprintln!(
            "CHM-{}: stdout flush failed: {}",
            HeatmapError::RptStdoutWriteFailed.code(),
            _io_error
        );
        return Err(HeatmapError::RptStdoutWriteFailed);
    }
    Ok(())
}

// ============================================================================
// Command line
// ============================================================================

/// Parses `--lcov`, `--out-dir`, `--top-n` (flag/value pairs; any order).
/// Generic over the iterator so tests can pass synthetic arguments.
fn parse_cli_arguments<I>(arguments: I) -> Result<HeatmapRunConfiguration, HeatmapError>
where
    I: Iterator<Item = OsString>,
{
    let mut remaining_arguments = arguments.skip(1); // skip program name
    let mut lcov_path_argument: Option<PathBuf> = None;
    let mut output_directory_argument: Option<PathBuf> = None;
    let mut top_n_argument: Option<u16> = None;
    let mut reached_end_of_arguments = false;

    for _flag_pair_index in 0..MAX_CLI_FLAG_PAIRS {
        let flag_argument = match remaining_arguments.next() {
            Some(flag) => flag,
            None => {
                reached_end_of_arguments = true;
                break;
            }
        };
        let value_argument = match remaining_arguments.next() {
            Some(value) => value,
            None => return Err(HeatmapError::CliMissingFlagValue),
        };

        match flag_argument.to_str() {
            Some("--lcov") => {
                if lcov_path_argument.is_some() {
                    return Err(HeatmapError::CliDuplicateFlag);
                }
                lcov_path_argument = Some(PathBuf::from(value_argument));
            }
            Some("--out-dir") => {
                if output_directory_argument.is_some() {
                    return Err(HeatmapError::CliDuplicateFlag);
                }
                output_directory_argument = Some(PathBuf::from(value_argument));
            }
            Some("--top-n") => {
                if top_n_argument.is_some() {
                    return Err(HeatmapError::CliDuplicateFlag);
                }
                let top_n_text = match value_argument.to_str() {
                    Some(text) => text,
                    None => return Err(HeatmapError::CliTopNNotUtf8),
                };
                match top_n_text.parse::<u16>() {
                    Ok(parsed_top_n) => top_n_argument = Some(parsed_top_n),
                    Err(_parse_error) => {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "CHM-{}: --top-n parse failed: {}",
                            HeatmapError::CliTopNNotANumber.code(),
                            _parse_error
                        );
                        return Err(HeatmapError::CliTopNNotANumber);
                    }
                }
            }
            _ => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: unknown flag: {:?}",
                    HeatmapError::CliUnknownFlag.code(),
                    flag_argument
                );
                return Err(HeatmapError::CliUnknownFlag);
            }
        }
    }

    if !reached_end_of_arguments && remaining_arguments.next().is_some() {
        return Err(HeatmapError::CliTooManyArguments);
    }

    let lcov_path = match lcov_path_argument {
        Some(path) => path,
        None => return Err(HeatmapError::CliMissingLcovPath),
    };
    let output_directory = match output_directory_argument {
        Some(path) => path,
        None => return Err(HeatmapError::CliMissingOutputDirectory),
    };

    Ok(HeatmapRunConfiguration {
        lcov_input_path: AbsolutePath::new(lcov_path)?,
        output_directory_path: AbsolutePath::new(output_directory)?,
        top_n: TopNCount::new(top_n_argument.unwrap_or(DEFAULT_TOP_N))?,
    })
}

// ============================================================================
// File helpers
// ============================================================================

/// Maps an I/O error kind to the caller's site-specific error codes.
fn classify_open_error(io_error_kind: std::io::ErrorKind, codes: OpenErrorCodes) -> HeatmapError {
    match io_error_kind {
        std::io::ErrorKind::NotFound => codes.not_found,
        std::io::ErrorKind::PermissionDenied => codes.permission_denied,
        std::io::ErrorKind::Interrupted
        | std::io::ErrorKind::WouldBlock
        | std::io::ErrorKind::TimedOut => codes.transient,
        _ => codes.other,
    }
}

/// Opens a file with Tier-1 bounded retry (only for retryable codes).
fn open_file_with_retry(
    file_path: &Path,
    purpose: FileOpenPurpose,
    codes: OpenErrorCodes,
) -> Result<File, HeatmapError> {
    for attempt_index in 0..MAX_OPEN_ATTEMPTS {
        let open_result = match purpose {
            FileOpenPurpose::ReadExisting => File::open(file_path),
            FileOpenPurpose::CreateTruncate => File::create(file_path),
        };
        match open_result {
            Ok(opened_file) => return Ok(opened_file),
            Err(io_error) => {
                let error_code = classify_open_error(io_error.kind(), codes);
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: open attempt {} failed for {}: {}",
                    error_code.code(),
                    attempt_index,
                    file_path.display(),
                    io_error
                );
                if !error_code.is_retryable() {
                    return Err(error_code);
                }
                let backoff_milliseconds = OPEN_RETRY_BACKOFF_STEP_MS
                    .saturating_mul(u64::from(attempt_index).saturating_add(1));
                std::thread::sleep(Duration::from_millis(backoff_milliseconds));
            }
        }
    }
    // Only retryable (transient) failures reach here: attempts exhausted.
    Err(codes.transient)
}

/// Creates/truncates a file and writes all bytes.
fn write_bytes_to_new_file(file_path: &Path, file_bytes: &[u8]) -> Result<(), HeatmapError> {
    let mut output_file = open_file_with_retry(
        file_path,
        FileOpenPurpose::CreateTruncate,
        REPORT_OPEN_ERROR_CODES,
    )?;
    if let Err(_io_error) = output_file.write_all(file_bytes) {
        #[cfg(debug_assertions)]
        eprintln!(
            "CHM-{}: write failed: {}",
            HeatmapError::RptFileWriteFailed.code(),
            _io_error
        );
        return Err(HeatmapError::RptFileWriteFailed);
    }
    if let Err(_io_error) = output_file.flush() {
        #[cfg(debug_assertions)]
        eprintln!(
            "CHM-{}: flush failed: {}",
            HeatmapError::RptFlushFailed.code(),
            _io_error
        );
        return Err(HeatmapError::RptFlushFailed);
    }
    Ok(())
}

/// Reads one line (including `\n`) into `line_buffer`, never more than
/// `MAX_LINE_BYTES` (+1 to detect overflow). Returns bytes read; 0 = EOF.
fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    line_buffer: &mut Vec<u8>,
    read_failed_code: HeatmapError,
    line_too_long_code: HeatmapError,
) -> Result<usize, HeatmapError> {
    line_buffer.clear();
    let read_limit = (MAX_LINE_BYTES as u64).saturating_add(1);
    let mut limited_reader = Read::take(&mut *reader, read_limit);
    match limited_reader.read_until(b'\n', line_buffer) {
        Ok(bytes_read) => {
            if bytes_read > MAX_LINE_BYTES {
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: line exceeds {} bytes",
                    line_too_long_code.code(),
                    MAX_LINE_BYTES
                );
                return Err(line_too_long_code);
            }
            Ok(bytes_read)
        }
        Err(_io_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: read failed: {}",
                read_failed_code.code(),
                _io_error
            );
            Err(read_failed_code)
        }
    }
}

/// Reads one source-file line for the loop scan. A line longer than
/// `MAX_LINE_BYTES` is NOT an error: its start is kept and its rest discarded.
///
/// # Project context
/// Some files in large projects are generated, with whole data tables written
/// on one line. In POC-1.1 one such line made the scanner skip the entire
/// file, so every loop in it became invisible (4 files in Pumpkin run 1).
/// The loop scan only ever looks at the *start* of a line (loop keywords,
/// `//`, `/*`), so keeping the start and discarding the rest loses nothing
/// the scan uses. The LCOV parser does not use this function: an over-long
/// LCOV line still means a corrupt file and stays fatal.
///
/// # Steps
/// 1. Read up to `MAX_LINE_BYTES + 1` bytes, stopping after a newline.
/// 2. 0 bytes: end of file. At most `MAX_LINE_BYTES` bytes: a complete line.
/// 3. More than `MAX_LINE_BYTES` bytes: an over-long line.
///    - If the last byte kept is the newline, the line has already ended
///      (it was exactly one byte too long). Nothing is discarded. Discarding
///      here would destroy the next line and shift every later line number.
///    - Otherwise discard up to and including the next newline (or end of
///      file) with `discard_rest_of_source_line`, checking for `*/`.
///
/// # Arguments
/// - `reader`: buffered source text (a file in production, a `Cursor` in tests).
/// - `line_buffer`: cleared, then filled with the line (or the line's start).
/// - `skip_cap_bytes`: maximum discarded remainder per line. Production passes
///   `MAX_SKIPPED_LINE_BYTES`; tests pass small values.
///
/// # Returns
/// - `Ok(SourceLineReadOutcome)`: see the enum for each case.
/// - `Err(SrcReadFailed)`: reading the line (or its start) failed.
/// - `Err(SrcLineExceedsSkipCap | SrcSkipReadFailed | SrcSkipReadCallLimit)`:
///   from `discard_rest_of_source_line`.
///
/// All errors are absorbed per file at Tier 2 by `scan_all_files_for_loops`.
///
/// # Modes
/// Debug builds print one diagnostic line per over-long line (no path).
fn read_source_line_skipping_overlong<R: BufRead>(
    reader: &mut R,
    line_buffer: &mut Vec<u8>,
    skip_cap_bytes: usize,
) -> Result<SourceLineReadOutcome, HeatmapError> {
    line_buffer.clear();
    let read_limit = (MAX_LINE_BYTES as u64).saturating_add(1);
    let kept_byte_count = {
        let mut limited_reader = Read::take(&mut *reader, read_limit);
        match limited_reader.read_until(b'\n', line_buffer) {
            Ok(bytes_read) => bytes_read,
            Err(_io_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: source read failed: {}",
                    HeatmapError::SrcReadFailed.code(),
                    _io_error
                );
                return Err(HeatmapError::SrcReadFailed);
            }
        }
    };

    if kept_byte_count == 0 {
        return Ok(SourceLineReadOutcome::EndOfInput);
    }
    if kept_byte_count <= MAX_LINE_BYTES {
        return Ok(SourceLineReadOutcome::CompleteLine);
    }

    #[cfg(debug_assertions)]
    eprintln!(
        "source line exceeds {} bytes: scanning its start, discarding the rest",
        MAX_LINE_BYTES
    );

    // The line ended exactly at the limit: nothing left to discard.
    if line_buffer.last() == Some(&b'\n') {
        return Ok(SourceLineReadOutcome::OverLongLineStartKept {
            discarded_part_contains_block_comment_end: false,
        });
    }

    // A `*` at the very end of the kept start may pair with a `/` at the very
    // beginning of the discarded part to form `*/`.
    let kept_part_ended_with_star = line_buffer.last() == Some(&b'*');
    let discarded_part_contains_block_comment_end =
        discard_rest_of_source_line(reader, kept_part_ended_with_star, skip_cap_bytes)?;
    Ok(SourceLineReadOutcome::OverLongLineStartKept {
        discarded_part_contains_block_comment_end,
    })
}

/// Reads and discards the rest of the current line, up to and including the
/// next newline (or end of file), and reports whether it contained `*/`.
///
/// # Project context
/// Used only for over-long source lines (see `read_source_line_skipping_overlong`).
/// The discarded bytes are never stored, so memory use stays constant no
/// matter how long the line is. The `*/` check keeps block-comment tracking
/// correct when a comment ends inside the discarded part.
///
/// # Steps (bounded loop)
/// 1. Ask the reader for its next chunk of buffered bytes (`fill_buf`).
///    - "Interrupted": try again (uses the extra read-call allowance).
///    - Empty chunk: end of file, so the line is finished.
/// 2. Take the chunk up to and including the first newline, or the whole
///    chunk if it has no newline.
/// 3. Check those bytes for `*/`, carrying "previous byte was `*`" across
///    chunks so a `*/` split between two chunks is still seen.
/// 4. Mark those bytes as consumed, and add them to the discarded total.
/// 5. If the total exceeds `skip_cap_bytes`: error. If a newline was
///    reached: done. Otherwise repeat.
///
/// The cap check comes before the newline check, so the result depends only
/// on the length of the remainder, not on how the reader splits it into chunks.
///
/// # Arguments
/// - `reader`: positioned just after the kept start of the over-long line.
/// - `kept_part_ended_with_star`: true if the last kept byte was `*`.
/// - `skip_cap_bytes`: maximum bytes to discard.
///
/// # Returns
/// - `Ok(true)` if `*/` occurred in the discarded part (or across the
///   kept/discarded boundary); `Ok(false)` otherwise.
/// - `Err(SrcSkipReadFailed)`: a read failed (other than "interrupted").
/// - `Err(SrcLineExceedsSkipCap)`: the remainder is longer than `skip_cap_bytes`.
/// - `Err(SrcSkipReadCallLimit)`: the read-call bound was reached (only
///   possible with endlessly interrupted reads; a safety bound).
fn discard_rest_of_source_line<R: BufRead>(
    reader: &mut R,
    kept_part_ended_with_star: bool,
    skip_cap_bytes: usize,
) -> Result<bool, HeatmapError> {
    // Each successful read call discards at least one byte, and the cap check
    // stops the loop once more than `skip_cap_bytes` have been discarded, so
    // at most `skip_cap_bytes + 1` successful calls occur. Interrupted calls
    // discard nothing and are covered by the extra allowance.
    let maximum_read_calls = skip_cap_bytes
        .saturating_add(1)
        .saturating_add(SKIP_LINE_EXTRA_READ_CALLS);
    let mut previous_byte_was_star = kept_part_ended_with_star;
    let mut block_comment_end_seen = false;
    let mut discarded_byte_count: usize = 0;

    for _read_call_index in 0..maximum_read_calls {
        let available_bytes = match reader.fill_buf() {
            Ok(bytes) => bytes,
            Err(io_error) if io_error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_io_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: read failed while discarding over-long line: {}",
                    HeatmapError::SrcSkipReadFailed.code(),
                    _io_error
                );
                return Err(HeatmapError::SrcSkipReadFailed);
            }
        };
        if available_bytes.is_empty() {
            // End of file inside the over-long line: the line is finished.
            return Ok(block_comment_end_seen);
        }

        let (consume_length, line_ended) =
            match available_bytes.iter().position(|byte| *byte == b'\n') {
                Some(newline_index) => (newline_index.saturating_add(1), true),
                None => (available_bytes.len(), false),
            };

        for &byte in available_bytes.iter().take(consume_length) {
            if previous_byte_was_star && byte == b'/' {
                block_comment_end_seen = true;
            }
            previous_byte_was_star = byte == b'*';
        }

        reader.consume(consume_length);
        discarded_byte_count = discarded_byte_count.saturating_add(consume_length);

        if discarded_byte_count > skip_cap_bytes {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: over-long line remainder exceeds {} bytes; skipping file",
                HeatmapError::SrcLineExceedsSkipCap.code(),
                skip_cap_bytes
            );
            return Err(HeatmapError::SrcLineExceedsSkipCap);
        }
        if line_ended {
            return Ok(block_comment_end_seen);
        }
    }

    #[cfg(debug_assertions)]
    eprintln!(
        "CHM-{}: discard read-call limit {} reached",
        HeatmapError::SrcSkipReadCallLimit.code(),
        maximum_read_calls
    );
    Err(HeatmapError::SrcSkipReadCallLimit)
}

// ============================================================================
// LCOV parsing
// ============================================================================

/// Parses an LCOV stream into per-file coverage records.
/// Recognized: `SF:`, `FN:`, `FNDA:`, `DA:`, `end_of_record`; others ignored.
fn parse_lcov_reader<R: BufRead>(reader: &mut R) -> Result<Vec<FileCoverageRecord>, HeatmapError> {
    let mut completed_records: Vec<FileCoverageRecord> = Vec::new();
    let mut current_record: Option<FileCoverageRecord> = None;
    let mut line_buffer: Vec<u8> = Vec::with_capacity(4096);
    let mut total_coverage_entries: usize = 0;

    for _lcov_line_index in 0..MAX_LCOV_LINES {
        let bytes_read = read_bounded_line(
            reader,
            &mut line_buffer,
            HeatmapError::LcovReadFailed,
            HeatmapError::LcovLineTooLong,
        )?;
        if bytes_read == 0 {
            return finish_lcov_parse(completed_records, current_record);
        }
        let lcov_line_text = match std::str::from_utf8(&line_buffer) {
            Ok(text) => text.trim_end(),
            Err(_utf8_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: lcov line not UTF-8: {}",
                    HeatmapError::LcovNotUtf8.code(),
                    _utf8_error
                );
                return Err(HeatmapError::LcovNotUtf8);
            }
        };
        parse_one_lcov_line(
            lcov_line_text,
            &mut current_record,
            &mut completed_records,
            &mut total_coverage_entries,
        )?;
    }
    Err(HeatmapError::LcovTooManyLines)
}

/// End-of-input checks for LCOV parsing.
fn finish_lcov_parse(
    completed_records: Vec<FileCoverageRecord>,
    current_record: Option<FileCoverageRecord>,
) -> Result<Vec<FileCoverageRecord>, HeatmapError> {
    if current_record.is_some() {
        return Err(HeatmapError::LcovUnterminatedRecord);
    }
    if completed_records.is_empty() {
        return Err(HeatmapError::LcovNoData);
    }
    Ok(completed_records)
}

/// Applies one LCOV line to the parser state.
fn parse_one_lcov_line(
    lcov_line_text: &str,
    current_record: &mut Option<FileCoverageRecord>,
    completed_records: &mut Vec<FileCoverageRecord>,
    total_coverage_entries: &mut usize,
) -> Result<(), HeatmapError> {
    if let Some(source_path_text) = lcov_line_text.strip_prefix("SF:") {
        if current_record.is_some() {
            return Err(HeatmapError::LcovMalformedRecord); // nested SF
        }
        if completed_records.len() >= MAX_SOURCE_FILES {
            return Err(HeatmapError::LcovTooManyFiles);
        }
        *current_record = Some(FileCoverageRecord::new(source_path_text));
        return Ok(());
    }

    if lcov_line_text == "end_of_record" {
        return match current_record.take() {
            Some(finished_record) => {
                completed_records.push(finished_record);
                Ok(())
            }
            None => Err(HeatmapError::LcovRecordOutsideFile),
        };
    }

    let is_used_entry = lcov_line_text.starts_with("FN:")
        || lcov_line_text.starts_with("FNDA:")
        || lcov_line_text.starts_with("DA:");
    if !is_used_entry {
        // TN, FNF, FNH, LF, LH, BRDA, BRF, BRH, blank: not used in POC-1.
        return Ok(());
    }

    let record = match current_record.as_mut() {
        Some(record) => record,
        None => return Err(HeatmapError::LcovRecordOutsideFile),
    };
    *total_coverage_entries = match total_coverage_entries.checked_add(1) {
        Some(new_total) if new_total <= MAX_COVERAGE_ENTRIES => new_total,
        _ => return Err(HeatmapError::LcovTooManyEntries),
    };

    if let Some(entry_text) = lcov_line_text.strip_prefix("FN:") {
        // FN:<line>,<function name>   (name may contain commas: split once)
        let (line_text, function_name) = match entry_text.split_once(',') {
            Some(parts) => parts,
            None => return Err(HeatmapError::LcovMalformedRecord),
        };
        let line_number = parse_lcov_line_number(line_text)?;
        record
            .function_definitions
            .push((line_number, function_name.to_string()));
    } else if let Some(entry_text) = lcov_line_text.strip_prefix("FNDA:") {
        // FNDA:<count>,<function name>
        let (count_text, function_name) = match entry_text.split_once(',') {
            Some(parts) => parts,
            None => return Err(HeatmapError::LcovMalformedRecord),
        };
        let entry_count = parse_lcov_count(count_text)?;
        let count_slot = record
            .function_entry_counts
            .entry(function_name.to_string())
            .or_insert(0);
        *count_slot = match count_slot.checked_add(entry_count) {
            Some(sum) => sum,
            None => return Err(HeatmapError::LcovCountOverflow),
        };
    } else if let Some(entry_text) = lcov_line_text.strip_prefix("DA:") {
        // DA:<line>,<count>[,<checksum>]
        let mut entry_fields = entry_text.splitn(3, ',');
        let line_text = match entry_fields.next() {
            Some(text) => text,
            None => return Err(HeatmapError::LcovMalformedRecord),
        };
        let count_text = match entry_fields.next() {
            Some(text) => text,
            None => return Err(HeatmapError::LcovMalformedRecord),
        };
        let line_number = parse_lcov_line_number(line_text)?;
        let line_count = parse_lcov_count(count_text)?;
        let count_slot = record.line_execution_counts.entry(line_number).or_insert(0);
        if line_count > *count_slot {
            *count_slot = line_count;
        }
        if line_count > record.max_line_execution_count {
            record.max_line_execution_count = line_count;
        }
    }
    Ok(())
}

/// Parses an LCOV line-number field into a validated `SourceLineNumber`.
fn parse_lcov_line_number(line_text: &str) -> Result<SourceLineNumber, HeatmapError> {
    let raw_line = match line_text.trim().parse::<u32>() {
        Ok(value) => value,
        Err(_parse_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: bad line number {:?}: {}",
                HeatmapError::LcovBadLineNumber.code(),
                line_text,
                _parse_error
            );
            return Err(HeatmapError::LcovBadLineNumber);
        }
    };
    match SourceLineNumber::new(raw_line) {
        Ok(line_number) => Ok(line_number),
        Err(_type_error) => Err(HeatmapError::LcovBadLineNumber),
    }
}

/// Parses an LCOV execution-count field.
fn parse_lcov_count(count_text: &str) -> Result<u64, HeatmapError> {
    match count_text.trim().parse::<u64>() {
        Ok(count) => Ok(count),
        Err(_parse_error) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: bad count {:?}: {}",
                HeatmapError::LcovBadCount.code(),
                count_text,
                _parse_error
            );
            Err(HeatmapError::LcovBadCount)
        }
    }
}

// ============================================================================
// Function sites
// ============================================================================

/// Builds one FUNCTION heat site per (source file, definition line).
///
/// # Project context
/// Function entry counts are the primary count-heat signal: exact LLVM
/// counters (not samples) for how often each function was entered during the
/// instrumented run. Identity is `source_path:line`, not the function name,
/// because (a) names may be mangled and (b) generic functions produce one
/// LCOV entry per monomorphized instance, all at the same source line.
///
/// # Merge rule
/// All instances defined at the same line are merged into one site: their
/// counts are summed and `merged_instance_count` records how many were merged.
/// The first instance's name is kept as `display_name`.
///
/// # Integrity check (duplicate names)
/// `FNDA` counts are stored by name, so two `FN` entries with the same name
/// in one file cannot be told apart: each would receive the shared total.
/// With mangled LCOV this does not happen. If the LCOV was demangled *before*
/// parsing, distinct instances can collapse to one name, which causes double
/// counting. The number of such duplicates is returned and shown in the
/// report's provenance block (expected: 0).
///
/// # Arguments
/// - `records`: parsed LCOV records, one per source file.
///
/// # Returns
/// - `Ok((function_sites, duplicate_function_name_count))`. Sites are in
///   LCOV order (NOT sorted): pass them to `sort_and_summarize_sites`.
/// - `Err(HeatmapError::AnaSiteIndexInvalid)`: internal index invariant broken.
/// - `Err(HeatmapError::AnaCountOverflow)`: merged count exceeded u64.
fn build_function_sites(
    records: &[FileCoverageRecord],
) -> Result<(Vec<HeatSite>, usize), HeatmapError> {
    let mut function_sites: Vec<HeatSite> = Vec::new();
    let mut duplicate_function_name_count: usize = 0;

    for (file_index, record) in records.iter().enumerate() {
        let mut site_index_by_line: HashMap<SourceLineNumber, usize> = HashMap::new();
        let mut function_names_seen: HashSet<&str> =
            HashSet::with_capacity(record.function_definitions.len());

        for (definition_line, function_name) in record.function_definitions.iter() {
            if !function_names_seen.insert(function_name.as_str()) {
                duplicate_function_name_count = duplicate_function_name_count.saturating_add(1);
            }

            let entry_count = record
                .function_entry_counts
                .get(function_name)
                .copied()
                .unwrap_or(0);

            match site_index_by_line.get(definition_line) {
                Some(&existing_site_index) => {
                    let existing_site = match function_sites.get_mut(existing_site_index) {
                        Some(site) => site,
                        None => {
                            #[cfg(all(debug_assertions, not(test)))]
                            debug_assert!(
                                false,
                                "CHM-{}: site index {} invalid",
                                HeatmapError::AnaSiteIndexInvalid.code(),
                                existing_site_index
                            );
                            #[cfg(debug_assertions)]
                            eprintln!(
                                "CHM-{}: site index {} invalid",
                                HeatmapError::AnaSiteIndexInvalid.code(),
                                existing_site_index
                            );
                            return Err(HeatmapError::AnaSiteIndexInvalid);
                        }
                    };
                    existing_site.execution_count =
                        match existing_site.execution_count.checked_add(entry_count) {
                            Some(sum) => sum,
                            None => return Err(HeatmapError::AnaCountOverflow),
                        };
                    existing_site.merged_instance_count =
                        existing_site.merged_instance_count.saturating_add(1);
                }
                None => {
                    let new_site_index = function_sites.len();
                    function_sites.push(HeatSite {
                        source_file_index: file_index,
                        line_number: *definition_line,
                        display_name: function_name.clone(),
                        execution_count: entry_count,
                        merged_instance_count: 1,
                        enclosing_function_entry_count: None,
                    });
                    site_index_by_line.insert(*definition_line, new_site_index);
                }
            }
        }
    }
    Ok((function_sites, duplicate_function_name_count))
}

// ============================================================================
// Loop sites (lexical scan, no AST)
// ============================================================================

/// Scans every non-cold source file for loop headers and collects LOOP sites.
///
/// # Project context
/// This is the "prioritization step" plus the per-file loop scan. A file
/// whose every LCOV line count is 0 cannot contain a hot loop, so it is
/// skipped without being opened (61% of files in Pumpkin run 1). This is how
/// the tool stays fast on very large codebases.
///
/// # Steps (per source file listed in the LCOV)
/// 1. Count the file in `files_total`.
/// 2. Skip it if it is cold (every line count is 0).
/// 3. Skip it if its LCOV path is not absolute.
/// 4. Otherwise open and scan it (`scan_one_source_file`).
///    - Success: add its loop sites and counters, including over-long lines.
///    - Failure (Tier 2): skip the file, count the reason, continue.
///
/// # Arguments
/// - `records`: parsed LCOV records, one per source file.
///
/// # Returns
/// `(loop_sites, statistics)`. Never fails: per-file problems are absorbed
/// and counted, so one unreadable file cannot stop the whole analysis.
/// Loop sites are in file order (NOT sorted): pass them to
/// `sort_and_summarize_sites`.
///
/// # Modes
/// Debug builds print the path and error for each skipped file. Production
/// builds print nothing; the counts appear in the report.
fn scan_all_files_for_loops(records: &[FileCoverageRecord]) -> (Vec<HeatSite>, LoopScanStatistics) {
    let mut all_loop_sites: Vec<HeatSite> = Vec::new();
    let mut scan_statistics = LoopScanStatistics::default();

    for (file_index, record) in records.iter().enumerate() {
        scan_statistics.files_total = scan_statistics.files_total.saturating_add(1);

        // Prioritization step: an entirely cold file cannot contain a hot loop.
        if record.max_line_execution_count == 0 {
            scan_statistics.files_skipped_cold =
                scan_statistics.files_skipped_cold.saturating_add(1);
            continue;
        }

        let absolute_source_path = match AbsolutePath::new(PathBuf::from(&record.source_path_text))
        {
            Ok(path) => path,
            Err(_type_error) => {
                scan_statistics.files_skipped_non_absolute_path = scan_statistics
                    .files_skipped_non_absolute_path
                    .saturating_add(1);
                continue;
            }
        };

        match scan_one_source_file(&absolute_source_path, file_index, record) {
            Ok(scan_outcome) => {
                scan_statistics.files_scanned = scan_statistics.files_scanned.saturating_add(1);
                scan_statistics.loop_headers_found = scan_statistics
                    .loop_headers_found
                    .saturating_add(scan_outcome.loop_headers_found);
                scan_statistics.loop_headers_without_line_data = scan_statistics
                    .loop_headers_without_line_data
                    .saturating_add(scan_outcome.loop_headers_without_line_data);
                scan_statistics.overlong_source_lines_total = scan_statistics
                    .overlong_source_lines_total
                    .saturating_add(scan_outcome.overlong_lines_found);
                if scan_outcome.overlong_lines_found > 0 {
                    scan_statistics.files_with_overlong_source_lines = scan_statistics
                        .files_with_overlong_source_lines
                        .saturating_add(1);
                }
                all_loop_sites.extend(scan_outcome.loop_sites);
            }
            Err(scan_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "loop scan skipped {}: {}",
                    record.source_path_text, scan_error
                );
                // Tier 2: absorb, count by reason, continue with the next file.
                match scan_error {
                    HeatmapError::SrcOpenNotFound => {
                        scan_statistics.files_skipped_not_found =
                            scan_statistics.files_skipped_not_found.saturating_add(1);
                    }
                    HeatmapError::SrcOpenPermissionDenied => {
                        scan_statistics.files_skipped_permission_denied = scan_statistics
                            .files_skipped_permission_denied
                            .saturating_add(1);
                    }
                    HeatmapError::SrcLineExceedsSkipCap => {
                        scan_statistics.files_skipped_line_exceeds_skip_cap = scan_statistics
                            .files_skipped_line_exceeds_skip_cap
                            .saturating_add(1);
                    }
                    HeatmapError::SrcTooManyLines => {
                        scan_statistics.files_skipped_too_many_lines = scan_statistics
                            .files_skipped_too_many_lines
                            .saturating_add(1);
                    }
                    _ => {
                        scan_statistics.files_skipped_other_error =
                            scan_statistics.files_skipped_other_error.saturating_add(1);
                    }
                }
            }
        }
    }
    (all_loop_sites, scan_statistics)
}

/// Opens one source file (Tier-1 retry) and scans it.
fn scan_one_source_file(
    absolute_source_path: &AbsolutePath,
    file_index: usize,
    record: &FileCoverageRecord,
) -> Result<SourceScanOutcome, HeatmapError> {
    let source_path = absolute_source_path.get()?;
    let source_file = open_file_with_retry(
        source_path,
        FileOpenPurpose::ReadExisting,
        SOURCE_OPEN_ERROR_CODES,
    )?;
    let mut source_reader = BufReader::new(source_file);
    scan_source_reader(&mut source_reader, file_index, record)
}

/// Scans one source file line by line for loop headers and builds LOOP sites.
///
/// # Project context
/// LLVM coverage has no "loop" concept, only per-line counts. This scan is
/// the lightweight (no AST) bridge: it finds lines that start a loop, and
/// takes that line's LCOV `DA` count as the loop's count. That count
/// approximates the number of iterations, which is what makes a loop hot
/// even inside a rarely called function.
///
/// # Per line
/// 1. Read the line with `read_source_line_skipping_overlong`. An over-long
///    line (more than `MAX_LINE_BYTES`) is counted in `overlong_lines_found`;
///    its kept start is then handled exactly like a normal line, so a loop
///    header at the start of a very long line is still found.
/// 2. Skip comment lines (`//` lines; `/* ... */` blocks that start a line).
///    For over-long lines, a `*/` in the discarded part also counts as the
///    end of the comment.
/// 3. Detect `for` / `while` / `loop` (optionally labeled) at line start.
/// 4. Look up the line's `DA` count. If there is none (not instrumented,
///    cfg'd out, or a lexical false positive), the header is counted in
///    `loop_headers_without_line_data` and skipped.
/// 5. Attribute the loop to the nearest preceding function definition line
///    (see `find_enclosing_function`). Store that function's summed entry
///    count, which the report uses for "iterations per call".
///
/// Line numbers stay exact: every line counts as one line, over-long or not.
///
/// # Arguments
/// - `source_reader`: buffered source text (a file in production, a `Cursor` in tests).
/// - `file_index`: index of `record` in the records slice (stored in each site).
/// - `record`: this file's LCOV data (function definitions, line counts).
///
/// # Returns
/// - `Ok(SourceScanOutcome)`: loop sites plus transparency counters.
/// - `Err(SrcReadFailed | SrcLineExceedsSkipCap | SrcSkipReadFailed |
///   SrcSkipReadCallLimit | SrcTooManyLines | SrcLineNumberOverflow |
///   AnaFunctionLineIndexOverflow | TypeLineNumberZero)`. The caller
///   (`scan_all_files_for_loops`) absorbs these at Tier 2: the file is
///   skipped, the reason is counted, and the analysis continues.
fn scan_source_reader<R: BufRead>(
    source_reader: &mut R,
    file_index: usize,
    record: &FileCoverageRecord,
) -> Result<SourceScanOutcome, HeatmapError> {
    let function_line_index = build_function_line_index(record)?;

    let mut scan_outcome = SourceScanOutcome::default();
    let mut line_buffer: Vec<u8> = Vec::with_capacity(256);
    let mut inside_block_comment = false;

    for line_index in 0..MAX_SOURCE_LINES_PER_FILE {
        let read_outcome = read_source_line_skipping_overlong(
            source_reader,
            &mut line_buffer,
            MAX_SKIPPED_LINE_BYTES,
        )?;
        // For complete lines nothing was discarded, so this is false.
        let discarded_part_contains_block_comment_end = match read_outcome {
            SourceLineReadOutcome::EndOfInput => return Ok(scan_outcome),
            SourceLineReadOutcome::CompleteLine => false,
            SourceLineReadOutcome::OverLongLineStartKept {
                discarded_part_contains_block_comment_end,
            } => {
                scan_outcome.overlong_lines_found =
                    scan_outcome.overlong_lines_found.saturating_add(1);
                discarded_part_contains_block_comment_end
            }
        };
        let trimmed_line = trim_ascii_whitespace_start(&line_buffer);

        // Crude comment handling (documented limitation: `/*` must start a line).
        if inside_block_comment {
            if contains_block_comment_end(trimmed_line) || discarded_part_contains_block_comment_end
            {
                inside_block_comment = false;
            }
            continue;
        }
        if trimmed_line.starts_with(b"//") {
            continue;
        }
        if trimmed_line.starts_with(b"/*") {
            let after_opener = trimmed_line.get(2..).unwrap_or(&[]);
            if !(contains_block_comment_end(after_opener)
                || discarded_part_contains_block_comment_end)
            {
                inside_block_comment = true;
            }
            continue;
        }

        let loop_keyword = match detect_loop_keyword(trimmed_line) {
            Some(keyword) => keyword,
            None => continue,
        };
        scan_outcome.loop_headers_found = scan_outcome.loop_headers_found.saturating_add(1);

        let one_based_line = match line_index.checked_add(1) {
            Some(value) => value,
            None => return Err(HeatmapError::SrcLineNumberOverflow),
        };
        let line_value = match u32::try_from(one_based_line) {
            Ok(value) => value,
            Err(_conversion_error) => return Err(HeatmapError::SrcLineNumberOverflow),
        };
        let line_number = SourceLineNumber::new(line_value)?;

        let header_line_count = match record.line_execution_counts.get(&line_number) {
            Some(&count) => count,
            None => {
                scan_outcome.loop_headers_without_line_data = scan_outcome
                    .loop_headers_without_line_data
                    .saturating_add(1);
                continue;
            }
        };

        let (enclosing_function_name, enclosing_function_entry_count) =
            match find_enclosing_function(&function_line_index, line_number) {
                Some(entry) => (entry.function_name, Some(entry.summed_entry_count)),
                None => ("<no enclosing fn found>", None),
            };

        let mut loop_display_name =
            String::with_capacity(enclosing_function_name.len().saturating_add(16));
        loop_display_name.push_str(loop_keyword.as_label());
        loop_display_name.push_str(" loop in ");
        loop_display_name.push_str(enclosing_function_name);

        scan_outcome.loop_sites.push(HeatSite {
            source_file_index: file_index,
            line_number,
            display_name: loop_display_name,
            execution_count: header_line_count,
            merged_instance_count: 1,
            enclosing_function_entry_count,
        });
    }
    Err(HeatmapError::SrcTooManyLines)
}

/// One function definition line in one source file, used only to attribute
/// loops to their enclosing function.
///
/// Entry counts of all LCOV instances at the same line are summed, which is
/// the same merge rule as function sites, so the per-call ratio uses the same
/// denominator the FUNCTIONS section shows. `function_name` borrows from the
/// `FileCoverageRecord`; the index lives only for the duration of one file scan.
#[derive(Debug, Clone, Copy)]
struct FunctionLineEntry<'name> {
    line_number: SourceLineNumber,
    function_name: &'name str,
    summed_entry_count: u64,
}

/// Builds the sorted, same-line-merged function index for one source file.
///
/// # Steps
/// 1. One entry per `FN` definition, with that instance's `FNDA` count
///    (0 if the instance has no `FNDA` entry).
/// 2. Sort by line number (required by the binary search in
///    `find_enclosing_function`).
/// 3. Merge adjacent entries on the same line by summing counts (generics).
///
/// # Arguments
/// - `record`: one file's LCOV data.
///
/// # Returns
/// - `Ok(entries)`: sorted ascending by line, one entry per distinct line.
/// - `Err(HeatmapError::AnaFunctionLineIndexOverflow)`: summed count exceeded u64.
fn build_function_line_index(
    record: &FileCoverageRecord,
) -> Result<Vec<FunctionLineEntry<'_>>, HeatmapError> {
    let mut unmerged_entries: Vec<FunctionLineEntry<'_>> = record
        .function_definitions
        .iter()
        .map(|(line_number, function_name)| FunctionLineEntry {
            line_number: *line_number,
            function_name: function_name.as_str(),
            summed_entry_count: record
                .function_entry_counts
                .get(function_name)
                .copied()
                .unwrap_or(0),
        })
        .collect();
    unmerged_entries.sort_by_key(|entry| entry.line_number);

    let mut merged_entries: Vec<FunctionLineEntry<'_>> = Vec::with_capacity(unmerged_entries.len());
    for entry in unmerged_entries {
        let same_line_as_last = merged_entries
            .last()
            .map(|last_entry| last_entry.line_number == entry.line_number)
            .unwrap_or(false);
        if same_line_as_last {
            if let Some(last_entry) = merged_entries.last_mut() {
                last_entry.summed_entry_count = match last_entry
                    .summed_entry_count
                    .checked_add(entry.summed_entry_count)
                {
                    Some(sum) => sum,
                    None => return Err(HeatmapError::AnaFunctionLineIndexOverflow),
                };
            }
        } else {
            merged_entries.push(entry);
        }
    }
    Ok(merged_entries)
}

/// Returns the function whose definition line is the nearest at or before
/// `line_number`, which is taken as the function enclosing that line.
///
/// # Approximation (documented limitation)
/// Without an AST there are no function end-lines, so "nearest preceding
/// definition" is used. This is usually right. It can be wrong for a loop
/// placed after a nested fn or closure that ends before the loop, in which
/// case the loop is attributed to that inner fn. The report flags the most
/// visible symptom: a counted loop whose attributed function has count 0
/// ("attribution suspect").
///
/// # Arguments
/// - `sorted_function_lines`: output of `build_function_line_index` (ascending).
/// - `line_number`: the loop header's line.
///
/// # Returns
/// - `Some(entry)`: a copy of the enclosing entry.
/// - `None`: no function is defined at or before this line (e.g. a loop in a
///   `const`/`static` initializer, or a lexical false positive near the top).
fn find_enclosing_function<'name>(
    sorted_function_lines: &[FunctionLineEntry<'name>],
    line_number: SourceLineNumber,
) -> Option<FunctionLineEntry<'name>> {
    let insertion_point =
        sorted_function_lines.partition_point(|entry| entry.line_number <= line_number);
    insertion_point
        .checked_sub(1)
        .and_then(|index| sorted_function_lines.get(index))
        .copied()
}

/// Detects a loop header at the start of an already left-trimmed line.
/// Accepts optional label (`'outer: for ...`). Rejects `for<'a>`, `forward`, `loopy`.
fn detect_loop_keyword(trimmed_line: &[u8]) -> Option<LoopKeyword> {
    let candidate_text = strip_loop_label(trimmed_line);
    if line_starts_with_keyword(candidate_text, b"for") {
        return Some(LoopKeyword::For);
    }
    if line_starts_with_keyword(candidate_text, b"while") {
        return Some(LoopKeyword::While);
    }
    if line_starts_with_keyword(candidate_text, b"loop") {
        return Some(LoopKeyword::Loop);
    }
    None
}

/// True if `text` starts with `keyword` followed by whitespace, end of text,
/// or (for `loop` only) an opening brace.
fn line_starts_with_keyword(text: &[u8], keyword: &[u8]) -> bool {
    if !text.starts_with(keyword) {
        return false;
    }
    match text.get(keyword.len()) {
        None => true,
        Some(&next_byte) => {
            next_byte.is_ascii_whitespace() || (keyword == b"loop" && next_byte == b'{')
        }
    }
}

/// Removes a leading loop label such as `'outer:` (and following whitespace).
/// Returns the input unchanged if no well-formed label is present.
fn strip_loop_label(text: &[u8]) -> &[u8] {
    if text.first() != Some(&b'\'') {
        return text;
    }
    let label_length = text
        .iter()
        .skip(1)
        .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_')
        .count();
    if label_length == 0 {
        return text;
    }
    let colon_index = match label_length.checked_add(1) {
        Some(index) => index,
        None => return text,
    };
    if text.get(colon_index) != Some(&b':') {
        return text;
    }
    let after_colon_index = match colon_index.checked_add(1) {
        Some(index) => index,
        None => return text,
    };
    match text.get(after_colon_index..) {
        Some(rest) => trim_ascii_whitespace_start(rest),
        None => text,
    }
}

/// Left-trims ASCII whitespace from a byte slice.
fn trim_ascii_whitespace_start(text: &[u8]) -> &[u8] {
    let first_non_whitespace = text
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(text.len());
    text.get(first_non_whitespace..).unwrap_or(&[])
}

/// True if the slice contains `*/`.
fn contains_block_comment_end(text: &[u8]) -> bool {
    text.windows(2).any(|window| window == b"*/")
}

// ============================================================================
// Ranking & summary
// ============================================================================

/// Sorts hottest first; deterministic tie-break by file index, then line.
fn sort_sites_hottest_first(sites: &mut [HeatSite]) {
    sites.sort_by(|left_site, right_site| {
        right_site
            .execution_count
            .cmp(&left_site.execution_count)
            .then_with(|| {
                left_site
                    .source_file_index
                    .cmp(&right_site.source_file_index)
            })
            .then_with(|| left_site.line_number.cmp(&right_site.line_number))
    });
}

/// Sorts one kind of site hottest-first, then summarizes it — always in that order.
///
/// # Project context
/// `summarize_sorted_sites` requires hottest-first input. Its concentration
/// figures ("the hottest K sites carry 50% / 90% / 99% of all counts") walk
/// the list from the top and record the first position where each threshold
/// is crossed. On unsorted input those figures are silently wrong: no error,
/// no panic, just misleading numbers in the report.
///
/// This function exists so that no caller can summarize without sorting.
/// (One POC-1.1 call path lost its sort during an edit; this closes that gap.)
///
/// # Arguments
/// - `sites`: all function sites, or all loop sites (one kind only; counts of
///   different kinds have different units and must not be mixed). The slice
///   is reordered in place; afterwards it is hottest-first with a
///   deterministic tie-break (file index, then line number).
///
/// # Returns
/// - `Ok(HeatKindSummary)` describing the now-sorted `sites`.
/// - `Err(HeatmapError::AnaCountOverflow)` or
///   `Err(HeatmapError::AnaHistogramIndexInvalid)`, propagated unchanged from
///   `summarize_sorted_sites`. Codes are unique, so `?` does not obscure the cause.
///
/// # Modes
/// No mode-specific behavior; both callees are panic-free.
fn sort_and_summarize_sites(sites: &mut [HeatSite]) -> Result<HeatKindSummary, HeatmapError> {
    sort_sites_hottest_first(sites);
    summarize_sorted_sites(sites)
}

/// Computes totals, zero-count sites, concentration, and decade histogram.
/// `sorted_sites` must be sorted hottest first.
fn summarize_sorted_sites(sorted_sites: &[HeatSite]) -> Result<HeatKindSummary, HeatmapError> {
    let mut summary = HeatKindSummary {
        site_count: sorted_sites.len(),
        zero_count_sites: 0,
        total_count: 0,
        sites_reaching_percent: [0; 3],
        decade_histogram: [0; DECADE_BUCKET_COUNT],
    };

    for site in sorted_sites.iter() {
        summary.total_count = match summary
            .total_count
            .checked_add(u128::from(site.execution_count))
        {
            Some(sum) => sum,
            None => return Err(HeatmapError::AnaCountOverflow),
        };
        if site.execution_count == 0 {
            summary.zero_count_sites = summary.zero_count_sites.saturating_add(1);
        }
        let bucket_index = match site.execution_count.checked_ilog10() {
            None => 0,
            Some(decimal_log) => (decimal_log as usize).saturating_add(1),
        };
        match summary.decade_histogram.get_mut(bucket_index) {
            Some(bucket_slot) => *bucket_slot = bucket_slot.saturating_add(1),
            None => return Err(HeatmapError::AnaHistogramIndexInvalid),
        }
    }

    if summary.total_count == 0 {
        return Ok(summary); // concentration is "n/a" (left as 0)
    }

    // threshold_i = total * target_percent_i (compare against cumulative * 100).
    let mut scaled_thresholds: [u128; 3] = [0; 3];
    for (target_index, target_percent) in CONCENTRATION_TARGETS_PERCENT.iter().enumerate() {
        let threshold = match summary.total_count.checked_mul(*target_percent) {
            Some(value) => value,
            None => return Err(HeatmapError::AnaCountOverflow),
        };
        if let Some(threshold_slot) = scaled_thresholds.get_mut(target_index) {
            *threshold_slot = threshold;
        }
    }

    let mut cumulative_count: u128 = 0;
    for (site_index, site) in sorted_sites.iter().enumerate() {
        cumulative_count = match cumulative_count.checked_add(u128::from(site.execution_count)) {
            Some(sum) => sum,
            None => return Err(HeatmapError::AnaCountOverflow),
        };
        let scaled_cumulative = match cumulative_count.checked_mul(100) {
            Some(value) => value,
            None => return Err(HeatmapError::AnaCountOverflow),
        };
        for (target_index, threshold) in scaled_thresholds.iter().enumerate() {
            if let Some(reached_slot) = summary.sites_reaching_percent.get_mut(target_index) {
                if *reached_slot == 0 && scaled_cumulative >= *threshold {
                    *reached_slot = site_index.saturating_add(1);
                }
            }
        }
    }
    Ok(summary)
}

/// Percent of `part` in `total` (0.0 when total is 0). Floating point: no panic.
fn percent_of_total(part: u64, total: u128) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (part as f64) * 100.0 / (total as f64)
}

/// Percent of `part` sites in `whole` sites (0.0 when whole is 0).
fn percent_of_site_count(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    (part as f64) * 100.0 / (whole as f64)
}

// ============================================================================
// Reports
// ============================================================================

/// Maps an in-memory report write failure to its code (debug detail only).
fn report_buffer_write_error(_io_error: std::io::Error) -> HeatmapError {
    #[cfg(debug_assertions)]
    eprintln!(
        "CHM-{}: report buffer write failed: {}",
        HeatmapError::RptBufferWriteFailed.code(),
        _io_error
    );
    HeatmapError::RptBufferWriteFailed
}

/// Resolves a site's file index to its source path text.
fn lookup_source_path(
    records: &[FileCoverageRecord],
    file_index: usize,
) -> Result<&str, HeatmapError> {
    match records.get(file_index) {
        Some(record) => Ok(record.source_path_text.as_str()),
        None => {
            #[cfg(all(debug_assertions, not(test)))]
            debug_assert!(
                false,
                "CHM-{}: file index {} invalid",
                HeatmapError::AnaFileIndexInvalid.code(),
                file_index
            );
            #[cfg(debug_assertions)]
            eprintln!(
                "CHM-{}: file index {} invalid",
                HeatmapError::AnaFileIndexInvalid.code(),
                file_index
            );
            Err(HeatmapError::AnaFileIndexInvalid)
        }
    }
}

/// Writes the human-readable text report.
///
/// # Project context
/// The report is meant to be read by a person deciding where to look in a
/// large codebase. It therefore starts with provenance (what was measured,
/// how, and what was left out) before any numbers, so every figure can be
/// traced back to its method and its known limitations.
///
/// # Sections, in order
/// 1. Title (states the phase: POC-1.2).
/// 2. `[provenance]`: input and output paths, measurement method, meaning of
///    each count, loop-detection method, over-long-line handling, and
///    integrity counters.
/// 3. `[loop-scan coverage]`: every LCOV source file counted as scanned or
///    under exactly one skip reason, plus over-long-line and loop-header counters.
/// 4. `[FUNCTIONS]` and `[LOOPS]` (see `write_heat_kind_section`).
/// 5. `[CO-COUNT GROUPS]` (see `write_co_count_groups`).
///
/// # Arguments
/// - `report_writer`: in-memory report buffer.
/// - `inputs`: all report data.
///
/// # Returns
/// - `Ok(())`.
/// - `Err(RptBufferWriteFailed)`, or an error code propagated from a section writer.
fn write_text_report<W: Write>(
    report_writer: &mut W,
    inputs: &ReportInputs,
) -> Result<(), HeatmapError> {
    let stats = inputs.loop_scan_statistics;
    writeln!(
        report_writer,
        "hix_heatmap_indices_for_compute :: POC-1.2 count-heat report"
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "==============================================="
    )
    .map_err(report_buffer_write_error)?;
    writeln!(report_writer).map_err(report_buffer_write_error)?;
    writeln!(report_writer, "[provenance: what was measured, and how]")
        .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  input lcov file         : {}",
        inputs.lcov_input_path.display()
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  report text file        : {}",
        inputs.report_file_path.display()
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  all-sites tsv file      : {}",
        inputs.sites_tsv_file_path.display()
    )
    .map_err(report_buffer_write_error)?;
    writeln!(report_writer, "  measurement method      : LLVM source-based coverage counters (rustc -C instrument-coverage); exact counts, not samples").map_err(report_buffer_write_error)?;
    writeln!(report_writer, "  run stop condition      : D = whenever the instrumented program exited cleanly (no in-tool cap)").map_err(report_buffer_write_error)?;
    writeln!(report_writer, "  function count meaning  : entry count (LCOV FNDA); monomorphized instances at one source line are summed").map_err(report_buffer_write_error)?;
    writeln!(report_writer, "  loop count meaning      : loop-header line execution count (LCOV DA); approximately the iteration count").map_err(report_buffer_write_error)?;
    writeln!(report_writer, "  per-call ratio meaning  : loop count / summed entry count of nearest preceding fn definition (approximate)").map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  loop detection method   : lexical line scan for for/while/loop at line start (no AST)"
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
            report_writer,
            "  over-long source lines  : first MAX_LINE_BYTES ({} bytes) scanned like a normal line; rest read and discarded (not an error)",
            MAX_LINE_BYTES
        )
        .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  time-heat               : NOT measured in POC-1.x"
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  source files in lcov    : {}",
        inputs.records.len()
    )
    .map_err(report_buffer_write_error)?;
    writeln!(report_writer, "  duplicate FN names      : {}  (expected 0; nonzero = names collapsed before parsing, function counts may be double-counted)", inputs.duplicate_function_name_count).map_err(report_buffer_write_error)?;
    writeln!(report_writer).map_err(report_buffer_write_error)?;

    writeln!(report_writer, "[loop-scan coverage]").map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files scanned", stats.files_scanned
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: cold (every line count = 0)", stats.files_skipped_cold
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: path not absolute", stats.files_skipped_non_absolute_path
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: not found", stats.files_skipped_not_found
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: permission denied", stats.files_skipped_permission_denied
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: line > MAX_SKIPPED_LINE_BYTES", stats.files_skipped_line_exceeds_skip_cap
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: too many lines", stats.files_skipped_too_many_lines
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files skipped: other error", stats.files_skipped_other_error
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "files with over-long lines (> MAX_LINE_BYTES)", stats.files_with_overlong_source_lines
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "over-long lines: start scanned, rest skipped", stats.overlong_source_lines_total
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "loop headers found", stats.loop_headers_found
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:<46}: {}",
        "loop headers with no line count", stats.loop_headers_without_line_data
    )
    .map_err(report_buffer_write_error)?;
    writeln!(report_writer).map_err(report_buffer_write_error)?;

    write_heat_kind_section(
        report_writer,
        "FUNCTIONS",
        "calls",
        inputs.function_sites,
        inputs.function_summary,
        inputs.records,
        inputs.top_n_limit,
    )?;
    writeln!(report_writer).map_err(report_buffer_write_error)?;
    write_heat_kind_section(
        report_writer,
        "LOOPS",
        "header-line executions",
        inputs.loop_sites,
        inputs.loop_summary,
        inputs.records,
        inputs.top_n_limit,
    )?;
    writeln!(report_writer).map_err(report_buffer_write_error)?;
    write_co_count_groups(report_writer, inputs)?;
    Ok(())
}

/// Writes one section (functions or loops): summary, concentration, cold
/// tail, order-of-magnitude histogram, and the Top-N table.
fn write_heat_kind_section<W: Write>(
    report_writer: &mut W,
    section_title: &str,
    count_unit: &str,
    sorted_sites: &[HeatSite],
    summary: &HeatKindSummary,
    records: &[FileCoverageRecord],
    top_n_limit: usize,
) -> Result<(), HeatmapError> {
    writeln!(
        report_writer,
        "[{}]  (count unit: {})",
        section_title, count_unit
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  sites: {}   total: {}   zero-count sites: {} ({:.2}%)",
        summary.site_count,
        summary.total_count,
        summary.zero_count_sites,
        percent_of_site_count(summary.zero_count_sites, summary.site_count)
    )
    .map_err(report_buffer_write_error)?;

    if summary.total_count == 0 {
        writeln!(report_writer, "  concentration: n/a (total count is 0)")
            .map_err(report_buffer_write_error)?;
    } else {
        for (target_index, target_percent) in CONCENTRATION_TARGETS_PERCENT.iter().enumerate() {
            let sites_needed = summary
                .sites_reaching_percent
                .get(target_index)
                .copied()
                .unwrap_or(0);
            writeln!(
                report_writer,
                "  {:>3}% of all counts come from the hottest {} sites ({:.3}% of sites)",
                target_percent,
                sites_needed,
                percent_of_site_count(sites_needed, summary.site_count)
            )
            .map_err(report_buffer_write_error)?;
        }
        let sites_for_99 = summary.sites_reaching_percent.get(2).copied().unwrap_or(0);
        let cold_tail_sites = summary.site_count.saturating_sub(sites_for_99);
        writeln!(
            report_writer,
            "  cold tail: {} sites ({:.2}% of sites) together carry <= 1% of counts",
            cold_tail_sites,
            percent_of_site_count(cold_tail_sites, summary.site_count)
        )
        .map_err(report_buffer_write_error)?;
    }

    writeln!(
        report_writer,
        "  order-of-magnitude histogram (non-empty buckets):"
    )
    .map_err(report_buffer_write_error)?;
    for (bucket_index, bucket_site_count) in summary.decade_histogram.iter().enumerate() {
        if *bucket_site_count == 0 {
            continue;
        }
        if bucket_index == 0 {
            writeln!(
                report_writer,
                "    count = 0            : {}",
                bucket_site_count
            )
            .map_err(report_buffer_write_error)?;
        } else {
            writeln!(
                report_writer,
                "    1e{:<2} <= count < 1e{:<2}: {}",
                bucket_index.saturating_sub(1),
                bucket_index,
                bucket_site_count
            )
            .map_err(report_buffer_write_error)?;
        }
    }

    writeln!(report_writer, "  top {} by count:", top_n_limit)
        .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  {:>5}  {:>20}  {:>9}  {:>6}  location  name",
        "rank", "count", "%kind", "log10"
    )
    .map_err(report_buffer_write_error)?;
    for (rank_index, site) in sorted_sites.iter().take(top_n_limit).enumerate() {
        let source_path_text = lookup_source_path(records, site.source_file_index)?;
        let line_value = site.line_number.get()?;
        let share_percent = percent_of_total(site.execution_count, summary.total_count);
        let rank = rank_index.saturating_add(1);
        if site.execution_count == 0 {
            write!(
                report_writer,
                "  {:>5}  {:>20}  {:>8.4}%  {:>6}  ",
                rank, site.execution_count, share_percent, "-"
            )
            .map_err(report_buffer_write_error)?;
        } else {
            write!(
                report_writer,
                "  {:>5}  {:>20}  {:>8.4}%  {:>6.2}  ",
                rank,
                site.execution_count,
                share_percent,
                (site.execution_count as f64).log10()
            )
            .map_err(report_buffer_write_error)?;
        }
        write!(
            report_writer,
            "{}:{}  {}",
            source_path_text, line_value, site.display_name
        )
        .map_err(report_buffer_write_error)?;
        if site.merged_instance_count > 1 {
            write!(
                report_writer,
                "  [{} instances merged]",
                site.merged_instance_count
            )
            .map_err(report_buffer_write_error)?;
        }
        if let Some(enclosing_entry_count) = site.enclosing_function_entry_count {
            if enclosing_entry_count == 0 {
                if site.execution_count > 0 {
                    // A counted loop inside a never-entered fn means the
                    // nearest-preceding-FN attribution picked the wrong fn.
                    write!(
                        report_writer,
                        "  [enclosing fn count 0: attribution suspect]"
                    )
                    .map_err(report_buffer_write_error)?;
                }
            } else {
                write!(
                    report_writer,
                    "  [~{:.1} per call of enclosing fn]",
                    (site.execution_count as f64) / (enclosing_entry_count as f64)
                )
                .map_err(report_buffer_write_error)?;
            }
        }
        writeln!(report_writer).map_err(report_buffer_write_error)?;
    }
    Ok(())
}

/// Writes the CO-COUNT GROUPS section: hot sites (functions and loops
/// together) whose exact counts are identical.
///
/// # Project context
/// LLVM counts are exact. When two sites at >= `CO_COUNT_MIN_COUNT` share
/// exactly the same count, coincidence is unlikely and structural coupling is
/// likely. Typical causes: a 1:1 caller/callee pair (e.g. `calc_distance`
/// calling `distance`), or a function called once per iteration of a loop.
/// Such groups are candidates for fusion or inlining review: a hint for a
/// human, not a verdict. Equal counts on code that ran across threads are
/// also mild evidence that counter-increment loss was negligible there.
///
/// # Method
/// 1. Collect all function and loop sites with count >= `CO_COUNT_MIN_COUNT`.
/// 2. Sort by count descending (tie-break: kind, file, line, so the output
///    is deterministic).
/// 3. Pass 1: find runs of identical counts of length >= 2.
/// 4. Pass 2: write the number of groups found, then up to `top_n_limit` groups.
///
/// # Arguments
/// - `report_writer`: in-memory report buffer.
/// - `inputs`: all report data (sites, records for path lookup, Top-N limit).
///
/// # Returns
/// - `Ok(())`.
/// - `Err(RptBufferWriteFailed | AnaCoCountRunInvalid | AnaFileIndexInvalid |
///   TypeLineNumberCorrupted)`, each from its own failure site.
fn write_co_count_groups<W: Write>(
    report_writer: &mut W,
    inputs: &ReportInputs,
) -> Result<(), HeatmapError> {
    let mut co_count_members: Vec<(&'static str, &HeatSite)> = Vec::new();
    for site in inputs.function_sites.iter() {
        if site.execution_count >= CO_COUNT_MIN_COUNT {
            co_count_members.push(("function", site));
        }
    }
    for site in inputs.loop_sites.iter() {
        if site.execution_count >= CO_COUNT_MIN_COUNT {
            co_count_members.push(("loop", site));
        }
    }
    co_count_members.sort_by(|(left_kind, left_site), (right_kind, right_site)| {
        right_site
            .execution_count
            .cmp(&left_site.execution_count)
            .then_with(|| left_kind.cmp(right_kind))
            .then_with(|| {
                left_site
                    .source_file_index
                    .cmp(&right_site.source_file_index)
            })
            .then_with(|| left_site.line_number.cmp(&right_site.line_number))
    });

    // Pass 1: find runs of identical counts (length >= 2). Bounded by list length.
    let mut run_ranges: Vec<(usize, usize)> = Vec::new();
    let mut run_start_index: usize = 0;
    for current_index in 1..=co_count_members.len() {
        let run_continues = match (
            co_count_members.get(current_index),
            co_count_members.get(run_start_index),
        ) {
            (Some((_, current_site)), Some((_, start_site))) => {
                current_site.execution_count == start_site.execution_count
            }
            _ => false,
        };
        if run_continues {
            continue;
        }
        if current_index.saturating_sub(run_start_index) >= 2 {
            run_ranges.push((run_start_index, current_index));
        }
        run_start_index = current_index;
    }

    // Pass 2: write.
    writeln!(
        report_writer,
        "[CO-COUNT GROUPS]  (identical exact counts >= {}; likely structural coupling)",
        CO_COUNT_MIN_COUNT
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  groups found: {}   (showing up to {})",
        run_ranges.len(),
        inputs.top_n_limit
    )
    .map_err(report_buffer_write_error)?;

    for (group_index, (run_start, run_end)) in
        run_ranges.iter().take(inputs.top_n_limit).enumerate()
    {
        let run_members = match co_count_members.get(*run_start..*run_end) {
            Some(members) => members,
            None => {
                #[cfg(all(debug_assertions, not(test)))]
                debug_assert!(
                    false,
                    "CHM-{}: co-count run {}..{} invalid",
                    HeatmapError::AnaCoCountRunInvalid.code(),
                    run_start,
                    run_end
                );
                #[cfg(debug_assertions)]
                eprintln!(
                    "CHM-{}: co-count run {}..{} invalid",
                    HeatmapError::AnaCoCountRunInvalid.code(),
                    run_start,
                    run_end
                );
                return Err(HeatmapError::AnaCoCountRunInvalid);
            }
        };
        let shared_count = run_members
            .first()
            .map(|(_, site)| site.execution_count)
            .unwrap_or(0);
        writeln!(
            report_writer,
            "  group {}: count {}, {} sites",
            group_index.saturating_add(1),
            shared_count,
            run_members.len()
        )
        .map_err(report_buffer_write_error)?;
        for (kind_label, site) in run_members.iter() {
            let source_path_text = lookup_source_path(inputs.records, site.source_file_index)?;
            let line_value = site.line_number.get()?;
            writeln!(
                report_writer,
                "    {:<8}  {}:{}  {}",
                kind_label, source_path_text, line_value, site.display_name
            )
            .map_err(report_buffer_write_error)?;
        }
    }
    Ok(())
}

/// Writes every site (functions first, then loops; each hottest-first) as TSV.
///
/// # Project context
/// The TSV is the machine-readable data product: complete (not Top-N), with
/// stable identity `source_path` + `line`, intended for POC-2 to join
/// time-heat (samply) onto count-heat.
///
/// # Column contract (append-only)
/// `kind, rank, count, percent_of_kind, log10_count, source_path, line, name,
/// merged_instances, per_enclosing_call`. New columns are only ever appended
/// at the end, so tools that read existing columns by position keep working.
/// Empty fields: `log10_count` when count is 0; `per_enclosing_call` for
/// function rows and for loops without a nonzero enclosing count.
/// Fields are not escaped (documented limitation).
///
/// # Returns
/// - `Ok(())`, or an error code from `write_tsv_rows` / `RptBufferWriteFailed`.
fn write_sites_tsv<W: Write>(
    tsv_writer: &mut W,
    inputs: &ReportInputs,
) -> Result<(), HeatmapError> {
    writeln!(
        tsv_writer,
        "kind\trank\tcount\tpercent_of_kind\tlog10_count\tsource_path\tline\tname\tmerged_instances\tper_enclosing_call"
    )
    .map_err(report_buffer_write_error)?;
    write_tsv_rows(
        tsv_writer,
        "function",
        inputs.function_sites,
        inputs.function_summary,
        inputs.records,
    )?;
    write_tsv_rows(
        tsv_writer,
        "loop",
        inputs.loop_sites,
        inputs.loop_summary,
        inputs.records,
    )?;
    Ok(())
}

/// Writes the TSV rows for one kind of site (see `write_sites_tsv` for the
/// column contract).
///
/// # Arguments
/// - `tsv_writer`: in-memory TSV buffer.
/// - `kind_label`: `"function"` or `"loop"` (first column).
/// - `sorted_sites`: one kind's sites, hottest-first (rank = position + 1).
/// - `summary`: that kind's summary (its total is the percent denominator).
/// - `records`: used to resolve each site's file index to its source path.
///
/// # Returns
/// - `Ok(())`.
/// - `Err(RptBufferWriteFailed | AnaFileIndexInvalid | TypeLineNumberCorrupted)`.
fn write_tsv_rows<W: Write>(
    tsv_writer: &mut W,
    kind_label: &str,
    sorted_sites: &[HeatSite],
    summary: &HeatKindSummary,
    records: &[FileCoverageRecord],
) -> Result<(), HeatmapError> {
    for (rank_index, site) in sorted_sites.iter().enumerate() {
        let source_path_text = lookup_source_path(records, site.source_file_index)?;
        let line_value = site.line_number.get()?;
        let share_percent = percent_of_total(site.execution_count, summary.total_count);
        write!(
            tsv_writer,
            "{}\t{}\t{}\t{:.6}\t",
            kind_label,
            rank_index.saturating_add(1),
            site.execution_count,
            share_percent
        )
        .map_err(report_buffer_write_error)?;
        if site.execution_count > 0 {
            write!(tsv_writer, "{:.4}", (site.execution_count as f64).log10())
                .map_err(report_buffer_write_error)?;
        }
        write!(
            tsv_writer,
            "\t{}\t{}\t{}\t{}\t",
            source_path_text, line_value, site.display_name, site.merged_instance_count
        )
        .map_err(report_buffer_write_error)?;
        if let Some(enclosing_entry_count) = site.enclosing_function_entry_count {
            if enclosing_entry_count > 0 {
                write!(
                    tsv_writer,
                    "{:.4}",
                    (site.execution_count as f64) / (enclosing_entry_count as f64)
                )
                .map_err(report_buffer_write_error)?;
            }
        }
        writeln!(tsv_writer).map_err(report_buffer_write_error)?;
    }
    Ok(())
}
// ============================================================================
// Tests (test mode only)
// ============================================================================
// ============================================================================
// Tests (test mode only)
// ============================================================================

/// Unit tests for POC-1.1 (compiled only under `cargo test`).
///
/// All tests use in-memory input (`std::io::Cursor`) and synthetic sites, so
/// they touch no files except `std::env::temp_dir()` path *values* (never
/// created or opened). `unwrap`/`assert!` are allowed here (test mode).
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// One file, two monomorphized instances of `tick` at line 1 (500 + 250
    /// calls), a cold `idle` at line 9, and a hot loop header at line 3
    /// (75_750 executions = 101 per call of `tick`).
    const SAMPLE_LCOV: &str = "TN:\n\
SF:/abs/src/world.rs\n\
FN:1,_ZN5world4tick17h0E\n\
FN:1,_ZN5world4tick17h1E\n\
FN:9,_ZN5world4idle17h2E\n\
FNDA:500,_ZN5world4tick17h0E\n\
FNDA:250,_ZN5world4tick17h1E\n\
FNDA:0,_ZN5world4idle17h2E\n\
FNF:3\n\
FNH:2\n\
DA:1,750\n\
DA:3,75750\n\
DA:9,0\n\
LF:3\n\
LH:2\n\
end_of_record\n";

    /// Source text matching `SAMPLE_LCOV`: one real loop at line 3, plus a
    /// loop keyword inside a line comment (line 2) and inside a block comment
    /// (lines 5-6) that must NOT be detected.
    const SAMPLE_SOURCE: &str = "fn tick() {\n\
    // for x in not_a_loop\n\
    for entity in entities {\n\
    }\n\
    /* while\n\
       loop */\n\
}\n\
\n\
fn idle() {}\n";

    /// Parses `SAMPLE_LCOV` into coverage records (panics on failure: test mode).
    fn parse_sample() -> Vec<FileCoverageRecord> {
        let mut cursor = Cursor::new(SAMPLE_LCOV.as_bytes());
        parse_lcov_reader(&mut cursor).unwrap()
    }

    /// Builds a synthetic site in file index 0 with the given count and line.
    /// `enclosing_function_entry_count` is `None` (function-site style).
    fn make_site(execution_count: u64, line: u32) -> HeatSite {
        HeatSite {
            source_file_index: 0,
            line_number: SourceLineNumber::new(line).unwrap(),
            display_name: String::from("test_site"),
            execution_count,
            merged_instance_count: 1,
            enclosing_function_entry_count: None,
        }
    }

    /// Renders the text report for synthetic site lists.
    /// Sites must already be hottest-first (callers pass sorted vectors).
    fn render_text_report(
        records: &[FileCoverageRecord],
        function_sites: &[HeatSite],
        loop_sites: &[HeatSite],
    ) -> String {
        let function_summary = summarize_sorted_sites(function_sites).unwrap();
        let loop_summary = summarize_sorted_sites(loop_sites).unwrap();
        let statistics = LoopScanStatistics::default();
        let report_path = std::env::temp_dir().join("r.txt");
        let tsv_path = std::env::temp_dir().join("s.tsv");
        let lcov_path = std::env::temp_dir().join("in.lcov");
        let inputs = ReportInputs {
            lcov_input_path: &lcov_path,
            report_file_path: &report_path,
            sites_tsv_file_path: &tsv_path,
            records,
            function_sites,
            function_summary: &function_summary,
            loop_sites,
            loop_summary: &loop_summary,
            loop_scan_statistics: &statistics,
            top_n_limit: 10,
            duplicate_function_name_count: 0,
        };
        let mut report_bytes: Vec<u8> = Vec::new();
        write_text_report(&mut report_bytes, &inputs).unwrap();
        String::from_utf8(report_bytes).unwrap()
    }

    /// LCOV parsing: one SF record with all FN/DA entries and the file max count.
    #[test]
    fn lcov_parse_reads_one_record() {
        let records = parse_sample();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_path_text, "/abs/src/world.rs");
        assert_eq!(records[0].function_definitions.len(), 3);
        assert_eq!(records[0].max_line_execution_count, 75_750);
    }

    /// Function sites: two instances at line 1 merge into one site (750 calls),
    /// and mangled names are unique, so the duplicate-name counter stays 0.
    #[test]
    fn function_sites_merge_monomorphized_instances() {
        let records = parse_sample();
        let (mut sites, duplicate_function_name_count) = build_function_sites(&records).unwrap();
        sort_sites_hottest_first(&mut sites);
        assert_eq!(duplicate_function_name_count, 0);
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].execution_count, 750);
        assert_eq!(sites[0].merged_instance_count, 2);
        assert_eq!(sites[0].enclosing_function_entry_count, None);
        assert_eq!(sites[1].execution_count, 0);
    }

    /// Loop scan: finds the real header at line 3 and ignores keywords in comments.
    #[test]
    fn loop_scan_finds_header_and_skips_comments() {
        let records = parse_sample();
        let mut source_cursor = Cursor::new(SAMPLE_SOURCE.as_bytes());
        let outcome = scan_source_reader(&mut source_cursor, 0, &records[0]).unwrap();
        assert_eq!(outcome.loop_headers_found, 1);
        assert_eq!(outcome.loop_sites.len(), 1);
        assert_eq!(outcome.loop_sites[0].execution_count, 75_750);
        assert_eq!(outcome.loop_sites[0].line_number.get().unwrap(), 3);
        assert!(outcome.loop_sites[0].display_name.contains("tick"));
    }

    /// Loop scan: the loop records its enclosing function's summed entry
    /// count (500 + 250), and the report shows the per-call ratio (101.0).
    #[test]
    fn loop_site_records_enclosing_function_count() {
        let records = parse_sample();
        let mut source_cursor = Cursor::new(SAMPLE_SOURCE.as_bytes());
        let outcome = scan_source_reader(&mut source_cursor, 0, &records[0]).unwrap();
        assert_eq!(
            outcome.loop_sites[0].enclosing_function_entry_count,
            Some(750)
        );
        let report_text = render_text_report(&records, &[], &outcome.loop_sites);
        assert!(report_text.contains("~101.0 per call of enclosing fn"));
    }

    /// Keyword detection: accepted loop forms and rejected look-alikes.
    #[test]
    fn loop_keyword_detection_cases() {
        assert_eq!(
            detect_loop_keyword(b"for x in items {"),
            Some(LoopKeyword::For)
        );
        assert_eq!(
            detect_loop_keyword(b"while let Some(x) = q.pop() {"),
            Some(LoopKeyword::While)
        );
        assert_eq!(detect_loop_keyword(b"loop {"), Some(LoopKeyword::Loop));
        assert_eq!(detect_loop_keyword(b"loop{"), Some(LoopKeyword::Loop));
        assert_eq!(
            detect_loop_keyword(b"'outer: loop {"),
            Some(LoopKeyword::Loop)
        );
        assert_eq!(
            detect_loop_keyword(b"'rows: for r in 0..n {"),
            Some(LoopKeyword::For)
        );
        assert_eq!(detect_loop_keyword(b"for<'a> Fn(&'a u8)"), None);
        assert_eq!(detect_loop_keyword(b"forward(x);"), None);
        assert_eq!(detect_loop_keyword(b"loopy();"), None);
        assert_eq!(detect_loop_keyword(b"'a'"), None);
        assert_eq!(detect_loop_keyword(b"let x = 1;"), None);
    }

    /// LCOV errors: each failure site returns its own unique code.
    #[test]
    fn lcov_errors_have_specific_codes() {
        let mut unterminated = Cursor::new("SF:/abs/a.rs\nDA:1,1\n".as_bytes());
        assert_eq!(
            parse_lcov_reader(&mut unterminated).err(),
            Some(HeatmapError::LcovUnterminatedRecord)
        );

        let mut outside = Cursor::new("DA:1,1\n".as_bytes());
        assert_eq!(
            parse_lcov_reader(&mut outside).err(),
            Some(HeatmapError::LcovRecordOutsideFile)
        );

        let mut empty = Cursor::new("".as_bytes());
        assert_eq!(
            parse_lcov_reader(&mut empty).err(),
            Some(HeatmapError::LcovNoData)
        );

        let mut bad_count = Cursor::new("SF:/abs/a.rs\nDA:1,abc\nend_of_record\n".as_bytes());
        assert_eq!(
            parse_lcov_reader(&mut bad_count).err(),
            Some(HeatmapError::LcovBadCount)
        );

        let mut zero_line = Cursor::new("SF:/abs/a.rs\nDA:0,1\nend_of_record\n".as_bytes());
        assert_eq!(
            parse_lcov_reader(&mut zero_line).err(),
            Some(HeatmapError::LcovBadLineNumber)
        );

        let long_line = "x".repeat(MAX_LINE_BYTES + 10);
        let mut too_long = Cursor::new(long_line.into_bytes());
        assert_eq!(
            parse_lcov_reader(&mut too_long).err(),
            Some(HeatmapError::LcovLineTooLong)
        );
    }

    /// Duplicate-name integrity check: two `FN` entries with one name in one
    /// file are counted (the condition that arises if LCOV is demangled first).
    #[test]
    fn duplicate_function_names_are_counted() {
        let lcov_text = "SF:/abs/a.rs\nFN:1,same\nFN:5,same\nFNDA:3,same\nDA:1,3\nend_of_record\n";
        let records = parse_lcov_reader(&mut Cursor::new(lcov_text.as_bytes())).unwrap();
        let (sites, duplicate_function_name_count) = build_function_sites(&records).unwrap();
        assert_eq!(sites.len(), 2);
        assert_eq!(duplicate_function_name_count, 1);
    }

    /// Summary on sorted input: totals, concentration, and decade histogram.
    #[test]
    fn summary_concentration_and_histogram() {
        let mut sites = vec![
            make_site(9, 2),
            make_site(90, 1),
            make_site(0, 4),
            make_site(1, 3),
        ];
        sort_sites_hottest_first(&mut sites);
        let summary = summarize_sorted_sites(&sites).unwrap();
        assert_eq!(summary.total_count, 100);
        assert_eq!(summary.zero_count_sites, 1);
        assert_eq!(summary.sites_reaching_percent, [1, 1, 2]);
        assert_eq!(summary.decade_histogram[0], 1); // count 0
        assert_eq!(summary.decade_histogram[1], 2); // 1 and 9
        assert_eq!(summary.decade_histogram[2], 1); // 90
    }

    /// Regression guard for the POC-1.1 missing-sort bug: unsorted input must
    /// produce the same concentration as sorted input.
    /// (Unsorted summarize of [1, 9, 0, 90] would wrongly report 4 sites for 50%.)
    #[test]
    fn sort_and_summarize_handles_unsorted_input() {
        let mut sites = vec![
            make_site(1, 3),
            make_site(9, 2),
            make_site(0, 4),
            make_site(90, 1),
        ];
        let summary = sort_and_summarize_sites(&mut sites).unwrap();
        assert_eq!(sites[0].execution_count, 90);
        assert_eq!(summary.sites_reaching_percent, [1, 1, 2]);
    }

    /// Summary when every count is 0: not an error; concentration is "n/a".
    #[test]
    fn summary_of_all_zero_counts_is_not_an_error() {
        let sites = vec![make_site(0, 1), make_site(0, 2)];
        let summary = summarize_sorted_sites(&sites).unwrap();
        assert_eq!(summary.total_count, 0);
        assert_eq!(summary.sites_reaching_percent, [0, 0, 0]);
    }

    /// Custom types reject out-of-range values at construction.
    #[test]
    fn custom_types_enforce_bounds() {
        assert_eq!(
            TopNCount::new(0).err(),
            Some(HeatmapError::TypeTopNOutOfRange)
        );
        assert_eq!(
            TopNCount::new(MAX_TOP_N + 1).err(),
            Some(HeatmapError::TypeTopNOutOfRange)
        );
        assert_eq!(TopNCount::new(5).unwrap().get().unwrap(), 5);
        assert_eq!(
            SourceLineNumber::new(0).err(),
            Some(HeatmapError::TypeLineNumberZero)
        );
        assert_eq!(
            AbsolutePath::new(PathBuf::from("relative/x.lcov")).err(),
            Some(HeatmapError::TypePathNotAbsolute)
        );
    }

    /// Command line: valid flags in any order produce a validated configuration.
    #[test]
    fn cli_parses_valid_arguments() {
        let lcov_path = std::env::temp_dir().join("chm_test_input.lcov");
        let output_path = std::env::temp_dir().join("chm_test_out");
        let arguments = vec![
            OsString::from("hix"), // correct? TODO check
            OsString::from("--lcov"),
            lcov_path.clone().into_os_string(),
            OsString::from("--out-dir"),
            output_path.into_os_string(),
            OsString::from("--top-n"),
            OsString::from("7"),
        ];
        let configuration = parse_cli_arguments(arguments.into_iter()).unwrap();
        assert_eq!(configuration.top_n.get().unwrap(), 7);
        assert_eq!(
            configuration.lcov_input_path.get().unwrap(),
            lcov_path.as_path()
        );
    }

    /// Command line: missing value, unknown flag, and relative path are rejected.
    #[test]
    fn cli_rejects_bad_arguments() {
        let missing_value = vec![OsString::from("bin"), OsString::from("--lcov")];
        assert_eq!(
            parse_cli_arguments(missing_value.into_iter()).err(),
            Some(HeatmapError::CliMissingFlagValue)
        );

        let unknown = vec![
            OsString::from("bin"),
            OsString::from("--nope"),
            OsString::from("x"),
        ];
        assert_eq!(
            parse_cli_arguments(unknown.into_iter()).err(),
            Some(HeatmapError::CliUnknownFlag)
        );

        let relative = vec![
            OsString::from("bin"),
            OsString::from("--lcov"),
            OsString::from("relative.lcov"),
            OsString::from("--out-dir"),
            std::env::temp_dir().into_os_string(),
        ];
        assert_eq!(
            parse_cli_arguments(relative.into_iter()).err(),
            Some(HeatmapError::TypePathNotAbsolute)
        );
    }

    /// Retry policy is defined per code: only transient open errors retry.
    #[test]
    fn retryability_is_per_code() {
        assert!(HeatmapError::LcovOpenTransient.is_retryable());
        assert!(!HeatmapError::LcovOpenNotFound.is_retryable());
        assert!(!HeatmapError::AnaCountOverflow.is_retryable());
    }

    /// End-to-end rendering: text report sections present; TSV has one header
    /// row plus one row per site (2 functions + 1 loop).
    #[test]
    fn text_report_and_tsv_render() {
        let records = parse_sample();
        let (mut function_sites, _duplicate_function_name_count) =
            build_function_sites(&records).unwrap();
        let function_summary = sort_and_summarize_sites(&mut function_sites).unwrap();
        let mut loop_sites = vec![make_site(75_750, 3)];
        let loop_summary = sort_and_summarize_sites(&mut loop_sites).unwrap();
        let statistics = LoopScanStatistics::default();
        let report_path = std::env::temp_dir().join("r.txt");
        let tsv_path = std::env::temp_dir().join("s.tsv");
        let lcov_path = std::env::temp_dir().join("in.lcov");
        let inputs = ReportInputs {
            lcov_input_path: &lcov_path,
            report_file_path: &report_path,
            sites_tsv_file_path: &tsv_path,
            records: &records,
            function_sites: &function_sites,
            function_summary: &function_summary,
            loop_sites: &loop_sites,
            loop_summary: &loop_summary,
            loop_scan_statistics: &statistics,
            top_n_limit: 10,
            duplicate_function_name_count: 0,
        };
        let mut report_bytes: Vec<u8> = Vec::new();
        write_text_report(&mut report_bytes, &inputs).unwrap();
        let report_text = String::from_utf8(report_bytes).unwrap();
        assert!(report_text.contains("[FUNCTIONS]"));
        assert!(report_text.contains("[LOOPS]"));
        assert!(report_text.contains("[CO-COUNT GROUPS]"));
        assert!(report_text.contains("2 instances merged"));

        let mut tsv_bytes: Vec<u8> = Vec::new();
        write_sites_tsv(&mut tsv_bytes, &inputs).unwrap();
        let tsv_text = String::from_utf8(tsv_bytes).unwrap();
        assert_eq!(tsv_text.lines().count(), 1 + 2 + 1);
        assert!(
            tsv_text
                .lines()
                .next()
                .unwrap()
                .ends_with("\tper_enclosing_call")
        );
    }

    /// Co-count groups: two hot groups are found (2 functions; 1 function +
    /// 1 loop); identical counts below CO_COUNT_MIN_COUNT are ignored.
    #[test]
    fn co_count_groups_detect_identical_hot_counts() {
        let records = parse_sample();
        let function_sites = vec![
            make_site(292_014_254, 1),
            make_site(292_014_254, 2),
            make_site(5_000_000, 3),
            make_site(7, 4),
            make_site(7, 5), // identical but below CO_COUNT_MIN_COUNT: ignored
        ];
        let loop_sites = vec![make_site(5_000_000, 9)];
        let report_text = render_text_report(&records, &function_sites, &loop_sites);
        assert!(report_text.contains("[CO-COUNT GROUPS]"));
        assert!(report_text.contains("groups found: 2"));
        assert!(report_text.contains("count 292014254, 2 sites"));
        assert!(report_text.contains("count 5000000, 2 sites"));
    }

    /// Parses LCOV text into records (panics on failure: test mode).
    fn parse_lcov_text(lcov_text: &str) -> Vec<FileCoverageRecord> {
        parse_lcov_reader(&mut Cursor::new(lcov_text.as_bytes())).unwrap()
    }

    /// POC-1.2-A acceptance test: a 70 KiB line no longer skips the file;
    /// the loop on the following line is found with the correct line number,
    /// count, and enclosing-function count.
    #[test]
    fn overlong_source_line_does_not_skip_file_and_next_loop_is_found() {
        let records = parse_lcov_text(
            "SF:/abs/src/gen.rs\nFN:1,_ZN3gen4fill17h0E\nFNDA:5,_ZN3gen4fill17h0E\nDA:1,5\nDA:3,500\nend_of_record\n",
        );
        let long_array_body = "1,".repeat(35 * 1024); // 71_680 bytes
        assert!(long_array_body.len() > MAX_LINE_BYTES);
        let source_text = format!(
            "fn fill() {{\n    const T: [u8; 1] = [{}];\n    for x in items {{\n    }}\n}}\n",
            long_array_body
        );
        let outcome =
            scan_source_reader(&mut Cursor::new(source_text.as_bytes()), 0, &records[0]).unwrap();
        assert_eq!(outcome.overlong_lines_found, 1);
        assert_eq!(outcome.loop_headers_found, 1);
        assert_eq!(outcome.loop_sites.len(), 1);
        assert_eq!(outcome.loop_sites[0].line_number.get().unwrap(), 3);
        assert_eq!(outcome.loop_sites[0].execution_count, 500);
        assert_eq!(
            outcome.loop_sites[0].enclosing_function_entry_count,
            Some(5)
        );
    }

    /// Edge case: a line exactly one byte too long, whose extra byte is its
    /// newline, has already ended. Nothing may be discarded, otherwise the
    /// next line (the loop) would be destroyed and line numbers would shift.
    #[test]
    fn line_one_byte_over_limit_does_not_swallow_next_line() {
        let records = parse_lcov_text(
            "SF:/abs/src/edge.rs\nFN:2,_ZN4edge1f17h0E\nFNDA:1,_ZN4edge1f17h0E\nDA:2,7\nend_of_record\n",
        );
        let mut source_text = "x".repeat(MAX_LINE_BYTES);
        source_text.push('\n');
        source_text.push_str("for y in z {\n}\n");
        let outcome =
            scan_source_reader(&mut Cursor::new(source_text.as_bytes()), 0, &records[0]).unwrap();
        assert_eq!(outcome.overlong_lines_found, 1);
        assert_eq!(outcome.loop_sites.len(), 1);
        assert_eq!(outcome.loop_sites[0].line_number.get().unwrap(), 2);
        assert_eq!(outcome.loop_sites[0].execution_count, 7);
    }

    /// A block comment whose `*/` is split exactly between the kept start
    /// (ending in `*`) and the discarded part (starting with `/`) is still
    /// seen as closed, so the loop after it is found.
    #[test]
    fn block_comment_end_split_across_discard_boundary_is_seen() {
        let records = parse_lcov_text(
            "SF:/abs/src/cmt.rs\nFN:1,_ZN3cmt1f17h0E\nFNDA:2,_ZN3cmt1f17h0E\nDA:4,9\nend_of_record\n",
        );
        // Line 3: MAX_LINE_BYTES 'x', then "*/". The kept start is
        // MAX_LINE_BYTES + 1 bytes, so it ends with '*'; the discarded part is "/\n".
        let mut source_text = String::from("fn f() {\n/* comment start\n");
        source_text.push_str(&"x".repeat(MAX_LINE_BYTES));
        source_text.push_str("*/\n");
        source_text.push_str("for a in b {\n}\n}\n");
        let outcome =
            scan_source_reader(&mut Cursor::new(source_text.as_bytes()), 0, &records[0]).unwrap();
        assert_eq!(outcome.overlong_lines_found, 1);
        assert_eq!(outcome.loop_sites.len(), 1);
        assert_eq!(outcome.loop_sites[0].line_number.get().unwrap(), 4);
    }

    /// A loop header at the start of an over-long line is detected from the
    /// kept start of the line.
    #[test]
    fn loop_header_at_start_of_overlong_line_is_detected() {
        let records = parse_lcov_text(
            "SF:/abs/src/big.rs\nFN:1,_ZN3big1g17h0E\nFNDA:3,_ZN3big1g17h0E\nDA:2,42\nend_of_record\n",
        );
        let source_text = format!(
            "fn g() {{\n    for v in [{}] {{\n    }}\n}}\n",
            "0,".repeat(35 * 1024)
        );
        let outcome =
            scan_source_reader(&mut Cursor::new(source_text.as_bytes()), 0, &records[0]).unwrap();
        assert_eq!(outcome.overlong_lines_found, 1);
        assert_eq!(outcome.loop_sites.len(), 1);
        assert_eq!(outcome.loop_sites[0].line_number.get().unwrap(), 2);
        assert_eq!(outcome.loop_sites[0].execution_count, 42);
    }

    /// Discarding: respects the cap, stops right after the newline (the next
    /// line stays readable), and detects `*/` including the carried `*`.
    #[test]
    fn discard_rest_of_source_line_respects_cap_and_stops_at_newline() {
        let mut no_newline = Cursor::new(b"xxxxxxxxxx".to_vec());
        assert_eq!(
            discard_rest_of_source_line(&mut no_newline, false, 4).err(),
            Some(HeatmapError::SrcLineExceedsSkipCap)
        );

        let mut with_newline = Cursor::new(b"abc*/def\nNEXT\n".to_vec());
        assert!(discard_rest_of_source_line(&mut with_newline, false, 100).unwrap());
        let mut next_line: Vec<u8> = Vec::new();
        with_newline.read_until(b'\n', &mut next_line).unwrap();
        assert_eq!(next_line, b"NEXT\n".to_vec());

        let mut carried_star = Cursor::new(b"/rest\n".to_vec());
        assert!(discard_rest_of_source_line(&mut carried_star, true, 100).unwrap());

        let mut no_comment_end = Cursor::new(b"*x/\n".to_vec());
        assert!(!discard_rest_of_source_line(&mut no_comment_end, false, 100).unwrap());
    }

    /// Single source of truth for error-code numbers.
    ///
    /// The enum discriminant is the only place a code number may be written.
    /// Every message must obtain it with `HeatmapError::Variant.code()`.
    /// This test reads this module and `main.rs` (compiled in as text) and
    /// fails if the marker "CHM-" is directly followed by a digit anywhere,
    /// naming the file and line. Renumbering codes therefore cannot leave a
    /// stale number behind in any message.
    #[test]
    fn source_contains_no_hard_coded_error_code_numbers() {
        let source_files: [(&str, &str); 2] = [
            (
                "hix_heatmap_indices_for_compute.rs",
                include_str!("hix_heatmap_indices_for_compute.rs"),
            ),
            ("main.rs", include_str!("main.rs")),
        ];
        let marker = "CHM-";
        for (file_name, source_text) in source_files.iter() {
            for (line_index, line_text) in source_text.lines().enumerate() {
                for (found_index, _) in line_text.match_indices(marker) {
                    let byte_after_marker = line_text.as_bytes().get(found_index + marker.len());
                    assert!(
                        !matches!(byte_after_marker, Some(byte) if byte.is_ascii_digit()),
                        "{}:{}: hard-coded error-code number: {}",
                        file_name,
                        line_index + 1,
                        line_text.trim()
                    );
                }
            }
        }
    }

    /// `USAGE_TEXT` is a static string (no heap, no formatting), so it cannot
    /// read `MAX_TOP_N` or `DEFAULT_TOP_N` directly. This test keeps it in
    /// step with those constants: changing a constant without updating the
    /// usage text fails here.
    #[test]
    fn usage_text_matches_top_n_constants() {
        let range_text = format!("--top-n 1..{}", MAX_TOP_N);
        let default_text = format!("(default {})", DEFAULT_TOP_N);
        assert!(
            USAGE_TEXT.contains(&range_text),
            "USAGE_TEXT range is stale"
        );
        assert!(
            USAGE_TEXT.contains(&default_text),
            "USAGE_TEXT default is stale"
        );
    }
}
