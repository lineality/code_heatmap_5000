//! # code_heatmap_5000 — POC-1: Primary Count-Heat (first slice of MVP 1.1)
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

use std::collections::HashMap;
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

/// Maximum bytes accepted for one line of LCOV or source text.
const MAX_LINE_BYTES: usize = 64 * 1024;

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
const REPORT_FILE_NAME: &str = "code_heatmap_5000_report.txt";

/// Output file name for the full per-site TSV data (inside `--out-dir`).
const SITES_TSV_FILE_NAME: &str = "code_heatmap_5000_sites.tsv";

/// Error log file name, created next to the executable (codes only, no PII).
const ERROR_LOG_FILE_NAME: &str = "code_heatmap_5000_error_log.txt";

/// Usage text shown by `main` for command-line errors (static: no heap).
pub const USAGE_TEXT: &str = "\
usage: code_heatmap_5000 --lcov /ABSOLUTE/path/file.lcov --out-dir /ABSOLUTE/output/dir [--top-n 1..10000]

  --lcov     LCOV file from: cargo llvm-cov report --lcov --output-path /ABSOLUTE/file.lcov
  --out-dir  directory for code_heatmap_5000_report.txt and code_heatmap_5000_sites.tsv
  --top-n    rows per Top-N table (default 20)
All paths must be absolute.";

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
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    SrcLineTooLong = 306,
    SrcTooManyLines = 307,
    SrcLineNumberOverflow = 308,

    // 400–499: analysis
    AnaCountOverflow = 401,
    AnaFileIndexInvalid = 402,
    AnaSiteIndexInvalid = 403,
    AnaHistogramIndexInvalid = 404,

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

            HeatmapError::CliTooManyArguments
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
            | HeatmapError::SrcLineTooLong
            | HeatmapError::SrcTooManyLines
            | HeatmapError::SrcLineNumberOverflow
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
            HeatmapError::CliTooManyArguments => "too many command-line arguments",
            HeatmapError::CliUnknownFlag => "unknown command-line flag",
            HeatmapError::CliMissingFlagValue => "flag given without a value",
            HeatmapError::CliMissingLcovPath => "--lcov is required",
            HeatmapError::CliMissingOutputDirectory => "--out-dir is required",
            HeatmapError::CliTopNNotUtf8 => "--top-n value is not UTF-8",
            HeatmapError::CliTopNNotANumber => "--top-n value is not a u16 number",
            HeatmapError::CliDuplicateFlag => "a flag was given more than once",
            HeatmapError::TypeTopNOutOfRange => "top-n must be 1..=10000",
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
            HeatmapError::SrcLineTooLong => "source line exceeds MAX_LINE_BYTES",
            HeatmapError::SrcTooManyLines => "source file exceeds MAX_SOURCE_LINES_PER_FILE",
            HeatmapError::SrcLineNumberOverflow => "source line number overflowed u32",
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
                "CHM-150: top-n {} outside 1..={}",
                candidate_top_n, MAX_TOP_N
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
            debug_assert!(false, "TopNCount corrupted: {}", self.top_n_value);
            #[cfg(debug_assertions)]
            eprintln!("CHM-151: TopNCount recheck failed: {}", self.top_n_value);
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
            eprintln!("CHM-152: line number 0 is invalid");
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
            debug_assert!(false, "SourceLineNumber corrupted: 0");
            #[cfg(debug_assertions)]
            eprintln!("CHM-153: SourceLineNumber recheck failed");
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
            eprintln!("CHM-154: not absolute: {}", candidate_path.display());
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
            debug_assert!(false, "AbsolutePath corrupted");
            #[cfg(debug_assertions)]
            eprintln!("CHM-155: AbsolutePath recheck failed");
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

/// Transparency counters for the loop scan (reported in the provenance block).
#[derive(Debug, Default, Clone, Copy)]
struct LoopScanStatistics {
    files_total: usize,
    files_scanned: usize,
    files_skipped_cold: usize,
    files_skipped_non_absolute_path: usize,
    files_skipped_unreadable: usize,
    loop_headers_found: usize,
    loop_headers_without_line_data: usize,
}

/// Result of scanning one source file.
#[derive(Debug, Default)]
struct SourceScanOutcome {
    loop_sites: Vec<HeatSite>,
    loop_headers_found: usize,
    loop_headers_without_line_data: usize,
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
#[derive(Debug, Clone, Copy)]
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
            eprintln!("CHM-601: current_exe failed: {}", _io_error);
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
            eprintln!("CHM-603: error log open failed: {}", _io_error);
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
            eprintln!("CHM-604: error log write failed: {}", _io_error);
            Err(HeatmapError::LogWriteFailed)
        }
    }
}

// ============================================================================
// Orchestration
// ============================================================================

/// Runs the full POC-1 pipeline for an already-validated configuration.
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

    // Step 2: function sites (exact entry counts).
    let mut function_sites = build_function_sites(&coverage_records)?;
    sort_sites_hottest_first(&mut function_sites);
    let function_summary = summarize_sorted_sites(&function_sites)?;

    // Step 3: loop sites (lexical scan; per-file failures absorbed at Tier 2).
    let (mut loop_sites, loop_scan_statistics) = scan_all_files_for_loops(&coverage_records);
    sort_sites_hottest_first(&mut loop_sites);
    let loop_summary = summarize_sorted_sites(&loop_sites)?;

    // Step 4: render reports into memory, then write files, then stdout.
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
    };

    let mut report_bytes: Vec<u8> = Vec::with_capacity(REPORT_BUFFER_INITIAL_CAPACITY);
    write_text_report(&mut report_bytes, &report_inputs)?;
    let mut sites_tsv_bytes: Vec<u8> = Vec::with_capacity(REPORT_BUFFER_INITIAL_CAPACITY);
    write_sites_tsv(&mut sites_tsv_bytes, &report_inputs)?;

    if let Err(_io_error) = fs::create_dir_all(output_directory_path) {
        #[cfg(debug_assertions)]
        eprintln!("CHM-501: create_dir_all failed: {}", _io_error);
        return Err(HeatmapError::RptCreateDirectoryFailed);
    }
    write_bytes_to_new_file(&report_file_path, &report_bytes)?;
    write_bytes_to_new_file(&sites_tsv_file_path, &sites_tsv_bytes)?;

    let standard_output = std::io::stdout();
    let mut standard_output_lock = standard_output.lock();
    if let Err(_io_error) = standard_output_lock.write_all(&report_bytes) {
        #[cfg(debug_assertions)]
        eprintln!("CHM-509: stdout write failed: {}", _io_error);
        return Err(HeatmapError::RptStdoutWriteFailed);
    }
    if let Err(_io_error) = standard_output_lock.flush() {
        #[cfg(debug_assertions)]
        eprintln!("CHM-509: stdout flush failed: {}", _io_error);
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
                        eprintln!("CHM-107: --top-n parse failed: {}", _parse_error);
                        return Err(HeatmapError::CliTopNNotANumber);
                    }
                }
            }
            _ => {
                #[cfg(debug_assertions)]
                eprintln!("CHM-102: unknown flag: {:?}", flag_argument);
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
        eprintln!("CHM-507: write failed: {}", _io_error);
        return Err(HeatmapError::RptFileWriteFailed);
    }
    if let Err(_io_error) = output_file.flush() {
        #[cfg(debug_assertions)]
        eprintln!("CHM-508: flush failed: {}", _io_error);
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
                eprintln!("CHM-208: lcov line not UTF-8: {}", _utf8_error);
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
            eprintln!("CHM-210: bad line number {:?}: {}", line_text, _parse_error);
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
            eprintln!("CHM-211: bad count {:?}: {}", count_text, _parse_error);
            Err(HeatmapError::LcovBadCount)
        }
    }
}

// ============================================================================
// Function sites
// ============================================================================

/// Builds one function site per (file, definition line), summing the entry
/// counts of all monomorphized instances defined at that line.
fn build_function_sites(records: &[FileCoverageRecord]) -> Result<Vec<HeatSite>, HeatmapError> {
    let mut function_sites: Vec<HeatSite> = Vec::new();

    for (file_index, record) in records.iter().enumerate() {
        let mut site_index_by_line: HashMap<SourceLineNumber, usize> = HashMap::new();

        for (definition_line, function_name) in record.function_definitions.iter() {
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
                            debug_assert!(false, "site index {} invalid", existing_site_index);
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
                    });
                    site_index_by_line.insert(*definition_line, new_site_index);
                }
            }
        }
    }
    Ok(function_sites)
}

// ============================================================================
// Loop sites (lexical scan, no AST)
// ============================================================================

/// Scans every non-cold source file for loop headers.
/// Tier 2: a file that cannot be scanned is skipped and counted; never fatal.
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
                all_loop_sites.extend(scan_outcome.loop_sites);
            }
            Err(_scan_error) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "loop scan skipped {}: {}",
                    record.source_path_text, _scan_error
                );
                scan_statistics.files_skipped_unreadable =
                    scan_statistics.files_skipped_unreadable.saturating_add(1);
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

/// Scans source text line by line for loop headers and attaches LCOV counts.
fn scan_source_reader<R: BufRead>(
    source_reader: &mut R,
    file_index: usize,
    record: &FileCoverageRecord,
) -> Result<SourceScanOutcome, HeatmapError> {
    // Function definition lines, sorted, for "enclosing function" attribution.
    let mut function_lines: Vec<(SourceLineNumber, &str)> = record
        .function_definitions
        .iter()
        .map(|(line, name)| (*line, name.as_str()))
        .collect();
    function_lines.sort_by_key(|(line, _name)| *line);

    let mut scan_outcome = SourceScanOutcome::default();
    let mut line_buffer: Vec<u8> = Vec::with_capacity(256);
    let mut inside_block_comment = false;

    for line_index in 0..MAX_SOURCE_LINES_PER_FILE {
        let bytes_read = read_bounded_line(
            source_reader,
            &mut line_buffer,
            HeatmapError::SrcReadFailed,
            HeatmapError::SrcLineTooLong,
        )?;
        if bytes_read == 0 {
            return Ok(scan_outcome);
        }
        let trimmed_line = trim_ascii_whitespace_start(&line_buffer);

        // Crude comment handling (documented limitation: `/*` must start a line).
        if inside_block_comment {
            if contains_block_comment_end(trimmed_line) {
                inside_block_comment = false;
            }
            continue;
        }
        if trimmed_line.starts_with(b"//") {
            continue;
        }
        if trimmed_line.starts_with(b"/*") {
            let after_opener = trimmed_line.get(2..).unwrap_or(&[]);
            if !contains_block_comment_end(after_opener) {
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
                // No region on this line: not instrumented, cfg'd out, or a
                // lexical false positive (e.g. text inside a string literal).
                scan_outcome.loop_headers_without_line_data = scan_outcome
                    .loop_headers_without_line_data
                    .saturating_add(1);
                continue;
            }
        };

        let enclosing_function_name = find_enclosing_function_name(&function_lines, line_number);
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
        });
    }
    Err(HeatmapError::SrcTooManyLines)
}

/// Returns the name of the nearest function defined at or before `line_number`.
fn find_enclosing_function_name<'a>(
    sorted_function_lines: &[(SourceLineNumber, &'a str)],
    line_number: SourceLineNumber,
) -> &'a str {
    let insertion_point = sorted_function_lines
        .partition_point(|(function_line, _name)| *function_line <= line_number);
    match insertion_point
        .checked_sub(1)
        .and_then(|index| sorted_function_lines.get(index))
    {
        Some((_function_line, function_name)) => function_name,
        None => "<no enclosing fn found>",
    }
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
    eprintln!("CHM-506: report buffer write failed: {}", _io_error);
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
            debug_assert!(false, "file index {} invalid", file_index);
            Err(HeatmapError::AnaFileIndexInvalid)
        }
    }
}

/// Writes the human-readable report: provenance, loop-scan coverage, then
/// one section each for functions (calls) and loops (header-line count).
fn write_text_report<W: Write>(
    report_writer: &mut W,
    inputs: &ReportInputs,
) -> Result<(), HeatmapError> {
    let stats = inputs.loop_scan_statistics;
    writeln!(
        report_writer,
        "code_heatmap_5000 :: POC-1 count-heat report"
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "============================================="
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
    writeln!(
        report_writer,
        "  loop detection method   : lexical line scan for for/while/loop at line start (no AST)"
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  time-heat               : NOT measured in POC-1"
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  source files in lcov    : {}",
        inputs.records.len()
    )
    .map_err(report_buffer_write_error)?;
    writeln!(report_writer).map_err(report_buffer_write_error)?;
    writeln!(report_writer, "[loop-scan coverage]").map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  files scanned                              : {}",
        stats.files_scanned
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  files skipped: cold (every line count = 0) : {}",
        stats.files_skipped_cold
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  files skipped: path not absolute           : {}",
        stats.files_skipped_non_absolute_path
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  files skipped: unreadable / read error     : {}",
        stats.files_skipped_unreadable
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  loop headers found                         : {}",
        stats.loop_headers_found
    )
    .map_err(report_buffer_write_error)?;
    writeln!(
        report_writer,
        "  loop headers with no line count            : {}",
        stats.loop_headers_without_line_data
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
        writeln!(report_writer).map_err(report_buffer_write_error)?;
    }
    Ok(())
}

/// Writes every site (not only Top-N) as TSV for later tools (POC-2 joins).
fn write_sites_tsv<W: Write>(
    tsv_writer: &mut W,
    inputs: &ReportInputs,
) -> Result<(), HeatmapError> {
    writeln!(
        tsv_writer,
        "kind\trank\tcount\tpercent_of_kind\tlog10_count\tsource_path\tline\tname\tmerged_instances"
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

/// Writes TSV rows for one kind of site.
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
        writeln!(
            tsv_writer,
            "\t{}\t{}\t{}\t{}",
            source_path_text, line_value, site.display_name, site.merged_instance_count
        )
        .map_err(report_buffer_write_error)?;
    }
    Ok(())
}

// ============================================================================
// Tests (test mode only)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// One file, two monomorphized instances of `tick` at line 1, a cold
    /// `idle` at line 9, and a hot loop header at line 3.
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

    const SAMPLE_SOURCE: &str = "fn tick() {\n\
    // for x in not_a_loop\n\
    for entity in entities {\n\
    }\n\
    /* while\n\
       loop */\n\
}\n\
\n\
fn idle() {}\n";

    fn parse_sample() -> Vec<FileCoverageRecord> {
        let mut cursor = Cursor::new(SAMPLE_LCOV.as_bytes());
        parse_lcov_reader(&mut cursor).unwrap()
    }

    fn make_site(execution_count: u64, line: u32) -> HeatSite {
        HeatSite {
            source_file_index: 0,
            line_number: SourceLineNumber::new(line).unwrap(),
            display_name: String::from("test_site"),
            execution_count,
            merged_instance_count: 1,
        }
    }

    #[test]
    fn lcov_parse_reads_one_record() {
        let records = parse_sample();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_path_text, "/abs/src/world.rs");
        assert_eq!(records[0].function_definitions.len(), 3);
        assert_eq!(records[0].max_line_execution_count, 75_750);
    }

    #[test]
    fn function_sites_merge_monomorphized_instances() {
        let records = parse_sample();
        let mut sites = build_function_sites(&records).unwrap();
        sort_sites_hottest_first(&mut sites);
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].execution_count, 750);
        assert_eq!(sites[0].merged_instance_count, 2);
        assert_eq!(sites[1].execution_count, 0);
    }

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

    #[test]
    fn summary_of_all_zero_counts_is_not_an_error() {
        let sites = vec![make_site(0, 1), make_site(0, 2)];
        let summary = summarize_sorted_sites(&sites).unwrap();
        assert_eq!(summary.total_count, 0);
        assert_eq!(summary.sites_reaching_percent, [0, 0, 0]);
    }

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

    #[test]
    fn cli_parses_valid_arguments() {
        let lcov_path = std::env::temp_dir().join("chm_test_input.lcov");
        let output_path = std::env::temp_dir().join("chm_test_out");
        let arguments = vec![
            OsString::from("code_heatmap_5000"),
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

    #[test]
    fn retryability_is_per_code() {
        assert!(HeatmapError::LcovOpenTransient.is_retryable());
        assert!(!HeatmapError::LcovOpenNotFound.is_retryable());
        assert!(!HeatmapError::AnaCountOverflow.is_retryable());
    }

    #[test]
    fn text_report_and_tsv_render() {
        let records = parse_sample();
        let mut function_sites = build_function_sites(&records).unwrap();
        sort_sites_hottest_first(&mut function_sites);
        let function_summary = summarize_sorted_sites(&function_sites).unwrap();
        let loop_sites = vec![make_site(75_750, 3)];
        let loop_summary = summarize_sorted_sites(&loop_sites).unwrap();
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
        };
        let mut report_bytes: Vec<u8> = Vec::new();
        write_text_report(&mut report_bytes, &inputs).unwrap();
        let report_text = String::from_utf8(report_bytes).unwrap();
        assert!(report_text.contains("[FUNCTIONS]"));
        assert!(report_text.contains("[LOOPS]"));
        assert!(report_text.contains("2 instances merged"));

        let mut tsv_bytes: Vec<u8> = Vec::new();
        write_sites_tsv(&mut tsv_bytes, &inputs).unwrap();
        let tsv_text = String::from_utf8(tsv_bytes).unwrap();
        assert_eq!(tsv_text.lines().count(), 1 + 2 + 1); // header + 2 functions + 1 loop
    }
}
