#### Rust Policy
- Mode & Case Handling
- Guidelines
- Rules

#### See 
- https://github.com/lineality/rust_lang_rules
- https://github.com/lineality/modes_and_case_handling
- https://github.com/lineality/modes_and_case_handling/blob/main/mode_case_handling_framework_summary.md

# HIX Heatmap Indices for Compute (Project Outline)
*"Somebody wakeup HIX."*

### Liebig-Barrel, Physical, Data, Code, Compute, Time, Memory Riparian Modeling

hix_heatmap_indices_for_compute.rs
hix.exe

##### 2026.11.01-... Geoffrey Gordon Ashbrook


#### Preamble Notes:

- Liebig's Barrel: https://en.wikipedia.org/wiki/Liebig%27s_law_of_the_minimum 

- Holzmann: https://spinroot.com/gerard/pdf/P10.pdf 

- HIX is focused on the Rust-language. It could be more abstractly (as another separate scope/project, or purely for comparison-planning-design discussion) for odin-lang, zig-lang, c, etc.



# Mapping Code Performance

Goal: A tool-set to analyze and report on code performance behavior to be able to improve code design.

Part of the context here is what might be seen as an array of ~modes, styles, and contexts, comparing and combining modes of what might be described as Development-Modes of code with (what might be described as) Implementation/Context type "modes."  By visualizing this design/development/deployment/improvement/maintenance problem-space landscape, we might be able to better focus on various needs and goals and their priorities. For example, what information/data are we using to navigate and decide within this problem space? This array can be seen as a data-information-question array: Do data compellingly suggest that there is work to be done (or not) in any of these areas? What are leads to look at? What are 'smoking guns'? Etc. What is ambiguous? What are tradeoffs, etc? 

| Implementation-Context Mode | Test / Analysis Mode | Debug Mode | Production Release Mode |
| :--- | :--- | :--- | :--- |
| **1. Classic Single-Thread, Single-Core, Scale-less Generic-Uniform-Load Mode** | | | |
| **2. Secure & Hot Regions Mode** | | | |
| **3. Multi-Core, Multi-Thread, Scale-Dependent Mode** | | | |
| **4. Stateless Parallel Lambda-Mode** *(e.g., Erlang-style for high telecom-scale)* | | | |
| **5. GPU / TPU / _PU Parallel Mode** | | | |


#### X axis: Holzmann-Type Development-Modes

1. Test/Analysis Mode
2. Debug Mode
3. Production Release Mode

#### Y Axis: Implementation-Context Modes

1. Classic Single-Thread, Single-Core, Scale-less Generic-Uniform-Load Mode: maximizing stack pre-allocation, minimizing heap allocation, etc.
2. Secure & Hot Regions Mode: Branchless conditionals, cache management, prediction-anticipation management, loop unrolling, optimized types and calculations, inline/naked assembly language, etc. 
3. Multi-Core, Multi-Thread, Scale-Dependent Mode
4. Stateless Parallel (CPU ?) Lambda-Mode (Where scale justifies creating maybe like erlang for telecom industry)
5. GPU/TPU/_PU Parallel Mode: Cuda Kernals & PTX, AMD ROCm, etc.; unified memory systems, etc. 

(x and y are arbitrary here.)



In this context, this idea is to (baby steps from crude starting point) explore systems, focusing on Rust, to get eval/analytics numbers on such things as:


# hix_heatmap_indices_for_compute: empirical and dynamic code analysis 

The 'indices' aspect of this, beyond providing various raw-data for human interpretation, is somewhat aspirational but perhaps can be achieved by disambiguating different areas for indices where what they represent can be notably different.


Raw-Type Indices:
1. Count-Heat-Index Per Loop & Function
2. Time-Heat-Index Per Loop & Function
3. Some Count Heat Function(s) Per Loop & Function
4. Inner-Element-Heat within Notable Loops & Functions




Interpretive Indices: Good vs. Bad Indices
A critical distinction between types of indices is the basic distinction between saying that a loop or function is prominent and should be vs. saying that a given region is prominent and it should not be, indicating that something should be changed. This may be similar to saying highly-represented in a neutral way, vs. something being over-represented in a pejorative way. 





0. Dashboard, Report-Organization: Useful presentation and exploration of data. E.g. Specific types and sub-types of head and tracing back what/where was measured how. (terminal/TUI, file(e.g. json), HTML, PDF)

1. Primary Count-Hot: execution frequency profiling (relative count-heat)

2. Secondary Time-Hot: execution time profiling (relative per-count time-heat)

3. Meta Heat, Count + Time: count alone and time alone may not surface combined-priminance.
e.g. ~ (maybe)
total_site_time ≈ site_call_count × avg_time_per_call
run_share  = total_site_time / total_time

#### performance quadrant:
- High count, high time/call
- High count, low time/call
- Low count, high time/call
- Low count, low time/call

4. (Optional/Theoretical) Tertiary Element-Hot: Specific Hot/Lag region analysis: what in/around/about a hot/lag function/loop is the issue? There may (or may not) be some cases where this is feasible; doubtless there are many cases where this is not feasible and unambiguous. 

5. (Very Theoretical) Supplemental: Suggestions: Specific Hot/Lag region analysis: what in/around/about a hot/lag function/loop is the solution? (Interesting to think about but probably only very rarely clear.)


Primary, Secondary & Meta data are the empirical experiments to generate data and standard-report those data: the data based on which decisions are made.

Tertiary and Supplemental are the fuzzy areas for human-use of the primary-secondary results, which in most cases will be too vague to automate. There may however be some common-cases where common-recommendations/best-practice are available.

These parts may also frame out MVP steps, with some Dashboard/Report (terminal/TUI, file(e.g. json), HTML, PDF) existing at each stage.

- MVP 1.1: Primary Count
- MVP 1.2: Secondary Time
- MVP 1.3: Meta-Data Calculations
- MVP 1.4: Tertiary
- MVP 1.5: Supplemental


It is somewhat open-ended how to use the quad-report, in terms of the overall scope of making a Liebig's Barrel model of how the code runs. Are there big gaps or only small ones? How many gaps are there? Is the barrel a normal barrel with microscopic gaps, or a ravaged-skeleton with vastly different cooper-lengths? Could/should this be scored? Is this unavoidable or preventable? etc.
 
1. There is some low-hanging fruit, such as top-N for three high areas, and maybe bottom-N for low-low. 
2. There is overall meta-data about that part of the quad (e.g. how much of the time/cpu/ram is 'high high' using? 1% vs. 99%? How many of the functions/loops are in each quad? etc.
3. There may also be other uses such as boundaries/clusters. E.g. possibly, in the case of a program that is already optimized the boundary/transition/riparian-zone between each quad might be 'smooth' and gradual. Whereas a program with some extremely un-optimized parts might have larger disjunction/clustering between the loops/functions that are in one or another quad. 

Ideally this could be tailored towards 'preventable issues to be fixed' vs. 'inevitable natural differences in loop/fn heat' 

Two possible 'modes' for using this tool are:
1. Looking at heat, which might or might not be natural differences (you can't tell unless you look) (maybe the default)
2. Identifying patterns or signatures that are not just 'places to look for issues that might not exist' but that are likely issues themselves detected directly. (maybe speculative)

#### Maybe: 
- Level 1 Macro (MVP): Function-Loop Level Maps, Scores
- Level 2 (Zoom-in): Inside Hot Functions & Loops: Maps, Scores


# MVP Overview:


#### count, report, and analyze these metrics:

1. raw count: how often each function is called 
2. raw count: how often is each region run
3. derived: Scale thresholds: Does the scale of use justify taking a different approach (or having an alternate path, e.g. the classic CPU vs. GPU load pivot, where above a given scale GPU is cheaper, but before that it is more expensive (maybe erlang type cpu/distributed lambda-function-abstraction is similar?)


#### Report can include:
1. raw count, percent, log per "function" 
2. raw count, percent, log per 'loop'/region
3. maybe included above or separate, some indication of use-thresholds... (e.g. enough scale for more-parallel use?)

Q: What represents a hot-region to be re-designed... maybe largely a human choice? or... some thresholds?

Q: detecting regions to track other than loops (or specific loops)

- Maybe some other analysis to see if a region is worth monitoring... 

e.g. Loop vs. Function:
A function may only run once but still contain a hot-loop
but a not-loop function... will at least on a first pass be counted as spotted? So maybe only loops and functions (not other regions) to be counted?



#### Primary Analysis: 
'heat map' of functions and loop counts: Relative Heat Counts, Logs, Percents, for N-time or N-max-loops (u32)

#### Secondary Analysis: 
Lag-Map: How-Slow: a super-duper-slow function/loop might not be called that many times because it is so slow. 

#### Meta-Analysis: 
Where to improve how?
In some cases another step of digging may be needed: e.g. If a given function is hot or laggy, what exactly in/about that function is the issue/solution?



### Analysis that scales with sanity: for small or large projects
- Tiny-Slim Projects: Tunnels: https://github.com/lineality/tunnels 
- Medium-Slim Projects: Lines, FF, Uma; 
https://github.com/lineality/lines_editor, 
https://github.com/lineality/ff_file_manager_minimal_rust 
- Variable Size Projects: https://github.com/lineality/granmo_model_nlp_classifier_rust 
- Larger Projects: https://github.com/pumpkin-mc/pumpkin


## Overall Workflow:

For design we will consider two general approaches, of which a hybrid may (or may not) eventually be used. 
1. Using Rust compile tools to inspect without modifying code.
2. Adding measurement-inspection code to a repo.


## Naive Approach 1:

Use a rust compiler flag 
```shell
-C instrument-coverage
```

to count each function/loop

This should produce a .profraw file.  
Then using an LLVM tool such as llvm-cov export or cargo llvm-cov --json, 
export/convert the .profraw file into readable json.
 

(note: this may not be able to count 'regions' of a function/loop specified for deeper analysis, which may eventually be needed to analyze and compare parts of a given function/loop)

For time, run the repo using a 'profiler' such as samply or perf.

'samply' exports to a Gecko JSON format. (maybe)
Maybe easier to use (linux, macOS(bsd), windows-os

'perf' exports using either a perf script or text report. (?)
output may be less standardized.

Both ~should record data including a usable/comparable source file path, line numbers and / or "function" names.

(note by claude:
```
To keep perf manageable:

POC-2a: self time only. Sample without call graphs and attribute each sample to its leaf frame. This is a tiny parse with small files, and it cannot fail because of unwinding.
POC-2b: inclusive time. Use frame-pointer call graphs (-C force-frame-pointers=yes), never DWARF unwinding. Cap the stack depth per sample, which keeps loops bounded.
```

## Naive Approach 2:

1. Make an ephemeral copy (or copies) of a repo
2. Inject into that test-repo time/count code to record analytics 
3. Crunch the numbers
4. Publish a report

e.g. deterministically generate a file-system and lookup for 

A. for each function & loop what is the path & code line-start

B. for each 'loop' what is the path & code-line start

maybe C. a quick loop-up table set to go from number/id to path

e.g. if counts are made by (separate theoretical options)

A. adding a counter directly to an audit structure in memory (arguably ideal in most cases), or

B. appending to a file, or (rarely or never needed)

C. creating a 'count' stub-file in a directory (may be async-safe) (rarely or never needed)


### Four Time Options

Another basic design question is how high/long to count.

Naive Count Parameter A: Count up to a given time-duration
Naive Count Parameter B: Count up to the first limited reached by any counter
Naive Count Parameter C: Both A and B, whichever happens first

Or it may be that A is better, assuming that potentially large counts are fine, because it will give a more accurate measure, e.g. 5-10 minutes of program run-time over various behaviors.


Naive Count Parameter D: If duration can be arbitrary, then whenever the program is stopped. 


### MVP approach to "Observer Effect" modeling and avoidance

compare minimal and complete versions of time-measurements:
1. coarse boundaries (functions, outer tasks), vs.
2. more extensive inspection

If there is a difference, that is noted. If there isn't a difference, then that is noted.


# Assumptions:
1. adding code and run-audits will slow down the application, but the idea is to analyse proportional use, not exact real-time

2. For counts, only function and loop counts are needed, not 'region' counts because (the assumption goes) region-counts are included in either function or loop counts. E.g. in terms of overall relative counts, a loop in a rarely called function will be hot, but there is no region in a cold function or cold loop that is hot.

3. semi-assumption: static-analysis will not necessarily tell you about these issues as clearly as empirical data (but some static-analysis might be useful too) ... or maybe that's wrong.

4. Functions here are assumed to be often large/long and performance (not aesthetics) oriented

5. Various existing tools are known/assumed to do some or all of various part of this scope:

- rust-code-analysis crate parses Rust source; reports each function time-span and loop constructs (FOR, WHILE, WHILES, …), with line numbers

- cargo flamegraph, flamegraph-rs, perf, samply: Lag-map

(cargo-perf?) https://docs.rs/cargo-perf/0.5.5/cargo_perf/ "Static analysis for async Rust performance anti-patterns.

cargo-perf complements clippy with checks specific to async code and common loop-related performance issues that clippy doesn’t catch."

- cargo llvm-cov / tarpaulin

- perf annotate: gives source-annotated sample counts

- cargo bench

- DHAT, heaptrack: memory lag analysis

- tokio-console: Async runtime attribution

- Criterion

- callgrind

- cargo-perf lints

- Clippy perf lints


6. A given 'region' of code may be implemented in various ways:
- recursion (banned here)
- "branched" conditional
- "branchless" conditional
- loops
- stateful
- stateless
- oop abstractions aspiring to negative-cost (but aspirations tend to be mental illness in disguise)
- raw assembler
- etc.


7. Some types/implementations of analysis do not scale well:

- AST: While I assume that some people will want to one-hammer-everything-nails turn this into an AST project, I want to look more closely at where this makes sense or not. E.g. for a small project an AST (or similar model) might help conceptually and practically. However, an initial larger-scale use-case for hix_heatmap_indices_for_compute is https://github.com/pumpkin-mc/pumpkin , where pumpkin-mc is so big that it literally crashes a better than normal computer to just open the zed editor when it runs the Rust Language Server (possibly: to build the AST) to look at a file. 

Part of the core-idea here is analysis that scales with sanity. 


8. Scale and separation of processes: It may be that a 'single' analysis is broken up into various parts and steps, such as 'prioritization' steps that eliminate the large numbers of cold functions and loops.

9. Where a time measure needs to be unaffected by the act of measuring then either a separate time-measure process step may be used or it may be an approximation or not possible, this should be clear in the data. E.g. the way that (maybe nested?) test-times may compound may make some times seem longer if they are not time-measured correctly. 

10. Async and counting:

Depending on the case for what is counted and how, in-memory counting may be the least-problematic, and it may be possible in many (not all) cases to count within async (with acceptable side-affects) using code such as this:

(use a HUGE grain of salt for the following brainstorm-code, these were suggested by Gemini)

(only to be used for multithreaded async (such as Tokyo))
```(grain of salt > from gemini-small-not-smart-model)
GLOBAL_COUNTERS[SITE_ID].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
```

vs.
Thread-Local Counters 
```(grain of salt > from gemini-small-not-smart-model)

Step 1 (The Pad): Every waiter has their own personal notepad in their pocket (THREAD_COUNTERS).

Step 2 (The Mark): Every time an order is placed, the waiter scribbles a tally 
mark on their own pad ((*THREAD_COUNTERS...)[SITE_ID] += 1). 

Step 3 (The Flush): At the end of the night, all waiters hand their pads to the manager, who sums up the totals onto a master board (GLOBAL AUDIT REPORT).
How they relate directly in code:

Part 1: The Declaration (Where the private counters live)
You define the thread-local storage once at the top of your program:

```rust
thread_local! {
    // Every OS thread automatically gets its own private copy of this array
    static THREAD_COUNTERS: std::cell::UnsafeCell<[u64; MAX_SITES]> = 
        std::cell::UnsafeCell::new([0; MAX_SITES]);
}

Part 2: The Action (Injected into your functions/loops)
Whenever a loop or function runs, it runs this line to increment its thread's private slot:
```rust
unsafe {
    (*THREAD_COUNTERS.with(|c| c.get()))[SITE_ID] += 1;
}

Part 3: The Flush (The collection step)
At program exit (or when a timer fires), you run a collection function like this:
```rust
// A central scoreboard shared across the whole app
static MASTER_TOTALS: [AtomicU64; MAX_SITES] = ...;

pub fn flush_thread_counters() {
    THREAD_COUNTERS.with(|local| {
        let counts = unsafe { &*local.get() };
        for (site_id, &count) in counts.iter().enumerate() {
            if count > 0 {
                // Add this thread's subtotal into the master scoreboard
                MASTER_TOTALS[site_id].fetch_add(count, Ordering::Relaxed);
            }
        }
    });
}
```

11. await time-architecture analysis: a possibly useful (and possibly possible) analysis area is the design and behavior of what is awaiting what for how long. All else optimized, an await-system itself may not be designed correctly.

(maybe... comparing time-stamps at either end of the awaited code?)

12. There may already be tools for this, but A. first, what is this, and B. are existing tools missing or deficient in anything?)

13. Goal: Map and Spot the low hanging Fruit
As with any "optimization" (or massive-non-optimization issue analysis) the first priority is low hanging fruit and overall perspective. 

Liebig's barrel: https://en.wikipedia.org/wiki/Liebig%27s_law_of_the_minimum  

This tool is not (currently) conceived of as being for cases where everything is already so optimized that it is nearly impossible to unambiguously detect any limiting factor (the barrel already has no non-microscopic gaps). This tool is mainly (currently) for getting some overall data where main-types of big patterns can be generally mapped.



# Advanced future scope and features:
- Elaborate multi-phase/step analysis
- Elaborate scale-difference handling and analysis
- myriad async related cases and edge cases
- dwelling on '"real" timing'(usually not a goal at all) vs. 'relative test time approximation'(goal)


14. Policy: While having no third-party dependencies is ideal, there is a good chance that well-tested utilities exist (one would hope!) and can be used or cover all or or some of this sketch of scope. It may be possible to use no 3rd party rust crates (or so up to a point).



# MVP-1 Scope:

1. Use perf tools to look at functions and loops, not (yet, if ever) file-changes for bespoke regions within functions and loops.
(Claude 5.5 is forever in a flame war with itself between using cargo/samply or perf... I cannot figure out why. cargo/samply maybe used later or in some other cases maybe)

2. Use/Output a parameter-specified Top-N quadrant report, e.g. overall data about each quadrant, and top-N examples of path-to-fn/loop in 3/4 quadrants.

3. For quad 4:4 (low,low) for which there is no need to give examples (though 
one or two might be academically useful in terms of looking at the bottom N lowest of the low...) the low/low portion of the quadrant could contain some overall metrics such as what portion of function / loops are in that group, maybe what portion of time is used by that group (which might be an indicator of how big the gaps are (e.g. if 99.9% of functions and loops are in low, low, that's probably a good sign... maybe). (Note: maybe there should be a high-level quad for overall data for all four regions?)

4. "You're a wizard, Heatmap!": Make a wizard_cli_module.rs, a Q&A bash-using module for (fedora) linux that can be called by main to semi-automate the overall process (including checking to see that things are installed). E.g. Q&A -> What is the path to the crate you want to test (required)? Is this default path to save the results-files ok (y/n)? [] Is this default path to save the report files ok (y/n)? [] etc. Store files in a standard modeling_data dir. (As the overall process grows, having a clear cli-wizard will be important for practical use, standardized use, and arguably repeat/audit/re-test use. (e.g. possibly having, saving and (re) running (sharing, etc) a wiz_config_{} file (whatever format). 

5. Time/duration of test probably should be included...

6. Choice of and use of perf over samply should be fully explained/documented.

7. No Yard Sale: All run/modeling files should be placed in / accessed from some standard dir such as /modeling_data/ or /modeling_data/ that is safely git-ignored: preferred 'modeling_data' or 'modeling_files' or just 'modeling'


# Not in MVP-1:
1. super-nuanced async/atomic/concurrent/parallel analysis


# Far Out of Scope (currently):
1. automated fixes or actions on original code itself (suggestions are in scope)
2. replacing or reinventing existing tools / features
3. languages other than Rust
4. a 'heavy AST', type-analysis, etc.
5. extremely arcane/exotic use-cases
6. per-line analysis is not anticipated (that might go along with only within specific regions within functions/loops, which is all future scope)


#### mode_case_handling_framework_summary



# Mode & Case Handling Bullet Points Summary
## Framework: Three Modes & Rules

### Three Modes of Operation:
1. Production-Release Mode: 
- Never panics, halts, or leaks data.
- Uses 2-byte error codes (no heap, no strings, no PII).
- Smoothly handles all cases via "Let It Fail & Recover" (with optional logging).
- Uses a Three-Level Recovery Hierarchy 

2. Debug Mode:
- Similar to production but logs verbose diagnostics (heap allowed).
- Uses `eprintln!` for errors and `debug_assert!` for internal invariants (gated to avoid running in test/production).
- Optional debug printing/logging for inspection.

3. Test Mode:
- Uses `assert!` in test functions.
- Deliberately crashes/panics to generate stack traces for debugging.



### Rules:

1. All functions return `Result<T, YourProjectError>`. The error payload is always the 2-byte `YourProjectError` enum. All operations are expected to eventually fail in some way; all such failures must be smoothly handled.

2. Production-Release Mode (Result-Error case-handling):
- never panics
- no heap in error-code-return from functions
- no print or log of that code by the function before the code is returned

3. Separate Debug-Mode Assertions from Cargo-Tests:
- `debug_assert!` must be gated with `#[cfg(all(debug_assertions, not(test)))]` to exclude from test mode and production mode builds.

4. Use Gated Verbose Debug-Mode Diagnostics:
- Use `eprintln!` (gated with `#[cfg(debug_assertions)]`) for debug-only output. Heap is allowed/needed here.

5. Test-Mode Isolation:
- Test code must be gated with `#[cfg(test)]`. 
- Avoid using test-mode tests inside functions for stable code.

6. Use a Fieldless Enum error-code system.

7. Use Enforced-Custom-Types for Value-Integrity.

8. Use a Three-Level Recovery Hierarchy for recovery and state.


### Enforced-Custom-Types & Value-Integrity:
- Use `struct`/`enum`/`impl` to enforce value boundaries for inputs and intermediate values. This ensures invalid states (including bit-flips, corruption) are caught and handled as errors, not passed silently.
- required: add a validity-recheck into the .get() method
- Optionally use a .validity_recheck() method
- While a custom type in Rust can enforce and validate that a value is within the definition boundaries, this check only happens once when the constructor is run ( .new() ), assuming that the constructor was correctly written to carry out that check correctly. If memory-corruption happens after that initial check there are no automatic checks that will catch that the value is now invalid (e.g. as the value is returned from and accepted by another function). Additional .validity_recheck() methods can be made and used to manually check at specific points.
- Using custom types helps to manage compatibility within the design (e.g. narrowing scope of function inputs)
- vanilla custom types (without additional validity-checks) guard against design errors (coding-mistakes) and 'expected' errors.
- additional validity-rechecks can add guards against some electrical, hardware, and adversarial based errors, or 'unexpected errors.'
- Getting and mutation/updating use methods that must be manually designed to check & enforce boundary checks and validity.
#### Public & Private
- note: the individual rules for structs, enums, impls, and their combinations, vary.
- A struct CAN be safely pub (so other modules can use the type name in signatures).
- The struct's inner fields should NOT be pub (so other modules cannot bypass checks).
- Example of safety for combinations: if an enum variant carries data then do not put raw unbounded primitives inside that; rather, put a bounded struct inside.

### Three-Level Recovery Hierarchy ("Let It Fail & Recover")
Production-release failures should fit one of three bounded recovery tiers:
- Recovery Tier 1: Micro-Retry (Local) — If err.is_retryable() is true, a caller repeats the bounded operation (with backoff/sleep).
- Recovery Tier 2: Step Fallback / Safe Degradation (Subsystem) — This is at a level above which any retry-errors would be thrown. If non-retryable, or retry attempts are exhausted, the subsystem safely aborts the current command, and handles what to do next. This is highly case dependent, and might include trying another function, reverting to a previous state, exiting silently, etc. Some internal state may need to be reset. Ultimately move on with or without logging the case or "error."
- Recovery Tier 3: Macro Re-initialization — An outer loop reinitializes state and continues execution without halting. The largest case for Tier 3 reboots the entire program.

#### Recovery & State-Recovery:
Recovery tiers represent functional layers of 'state' in terms of 'recovery levels' to plan for, e.g. what to do when some state-values may not exist. Tier 1 covers the state in question. Tiers 2 and 3 range from small to large 'reboot/retry' scales up to the whole program. Full system restart will likely often have at least some state that it can be restarted with (resulting in no noticeable interruption), sometimes implying that state should be managed outside of retry-loops, to retain intact values.


### Error Handling System
- Single Fieldless Enum:
  A global `YourProjectError` enum (e.g., `#[repr(u16)]`) defines all error codes.
  Properties:
  - No heap (2-byte `Copy` values).
  - No `String` or `io::Error` payloads (prevents PII leaks).
  - Exhaustive `match` for retryability (`is_retryable()` method).
  - Append-only codes: Never reuse or renumber.

- Error Code Table Rules:
  1. Codes are unique and permanent (e.g., `Fs32tFromStrInputTooLong = 101`).
  2. Reserve blocks per module/feature (e.g., 100–199 for `FixedSize32Timestamp`).
  3. Variant names: `AcronymFunctionCondition` (e.g., `IoPermissionDenied`).
  4. Document codes in the enum’s doc comment.
  5. Codes are single-source-of-truth, no hard-coding a 2nd-source of truth for what a code is presumed to be.

- Display for Debug Only:
  `Display` impl (for human-readable text) is compiled only for debug/test builds.

#### Error Sites & Propagation:
- Unique codes permit use of '?' because the unique code of the root-cause error is preserved (not obscured) by propagating the error.
- If an error site is of note, that site needs to throw a specific error code.
- This acts as a proxy for manually checking for most internal-invariant type issues.


### Two Detection Patterns
1. If-Detection-Pattern:
   For direct condition checks (e.g., `if len > 32`).
   - Debug: `debug_assert!` (internal invariants only) + `eprintln!`.
   - Production: Return terse error code.

2. Match-A-Function-Call-Pattern:
   For handling `Result` from fallible calls (e.g., `std::str::from_utf8`).
   - Debug: `debug_assert!(false, ...)` (for internal invariants) + `eprintln!`.
   - Production: Drop callee’s error (to avoid heap/PII), return project error code.


## Key Principles
- No Heap in Production: Ban `String`, `format!`, `Box<dyn Error>`, etc.
- No Panics in Production: Use `checked_add`, `.get()`, etc., to avoid silent wraps/panics.
- Defensive Programming:
  - Input Validation: Expected issues (e.g., bad user input) → return error code + debug `eprintln!`.
  - Internal Invariant: "Should-not-happen" checks (e.g., bit-flips) → `debug_assert!` + production catch.
- Retry Logic: Defined per error code (not ranges) in `is_retryable()`.


### Example Code
#### Error Enum
```rust
#[repr(u16)]
pub enum YourProjectError {
    IoNotFound = 50,
    Fs32tFromStrInputTooLong = 101,
    RetryMaxAttemptsZero = 200,
}
```

#### If-Detection-Pattern
```rust
if !condition {
    #[cfg(debug_assertions)]
    eprintln!("ACRO-101: detail: {}", value);
    return Err(YourProjectError::AcroFnCondition);
}
```

#### Match-A-Function-Call-Pattern
```rust
match fallible_call(input) {
    Ok(v) => Ok(v),
    Err(_detail) => {
        #[cfg(debug_assertions)]
        eprintln!("ACRO-101: {}", _detail);
        Err(YourProjectError::AcroFnCondition)
    }
}
```

### Banned in Production
- `unwrap`/`expect`/`panic!`
- Heap allocations (`String`, `format!`, `Box<dyn Error>`)
- Error messages with PII (file/dir paths, user data, etc.)
- Unchecked Arithmetic: Arithmetic operators that can panic in production (use `checked_add`, etc.)


#### Suggestions (TODO: under construction; move to rules, for manageable use of codes?)
- Allow effective "blocks" of codes for functions, to allow coherent numbering and easy incrementing in case two developers collide. e.g. if the last two digits are internal to a function, u16 would allow for 654 functions to each have 99 internal errors (or depending on average function size, ten per function may be enough on average).
- alt phrasing: allow 10 or 100 error-codes per function to manage allocations and changes to codes


#### See:
- https://github.com/lineality/rust_lang_rules


(Under Construction)
### Modes & Case Handling Framework 
2026 Geoffrey Gordon Ashbrook

# Four Levels of State Recovery:
1. Error Correcting Code
2. Single Value Retry/Recovery
3. Block/Process Retry/Recovery
4. Larger-Scale recovery loops, and state used, including the entire application.

Where standard type of recovery-handling are:
1. Panic & core-dump (not for production-release)
2. Return an Error (that then needs to be handled/recovered-from)
3. Reconstruct state from other data and move on. (e.g. error-correcting codes)
4. Re-try an earlier process (N times)
5. Use a default value and move on.
6. Do nothing and move on. (e.g. if the process was a 'window-dressing' feature that in no way cascades issues for future state)


TODO:
- overall bullet points
- rule 5
- example of 2-3 functions + error structures
(maybe unified-example instead of abstract separate examples)


1. short bullet-point version before explanation
2. Having every function-call-result be defined by an enum/struct (some of which may be re-used (e.g. 'can't be zero')
3. examples

Idea: 
A. emphasize using an enum/struct/impl (case by case varies) to define the required structure and value boundaries of all function inputs, 
B. (maybe a separate topic) this a way to enforce handling of 'should not happen' value-changes (such as bit-flips, hardware issues, or adversarial attacks) that could happen to values as they move through functions (e.g. after any given check-point and thereby not caught. Would a struct/impl/enum system require that the value-bounds are continually enforced, allowing a production-safe no-panic/no-crash smooth-handling of all such cases.


Note: this might also simplify value and input-checking... (more details on how?)

New 'rules for modes' rule:
debug-mode can try to 'assert'(panic) expected or unexpected issues

Rule 5. value-integrity via enforced-custom-types
integrity of value-parameters required for inputs to functions, and for values generated by function-calls within functions, should use structure-enforced-types using structs, impls, and enums (where appropriate), so that any (expected or 'cannot-happen') issue-case-error with that value will automatically trigger a handled-case/error-result and not silently pass along invisibly-invalid data-values.

or

Rule 5. Value Integrity via Enforced Custom Types:
   Use `struct`/`enum`/`impl` to enforce value boundaries for inputs and intermediate values.
   Ensures invalid states (e.g., bit-flips, corruption) are caught and handled as errors, not passed silently.

or

Framing Rule 5: Value-Integrity via Enforced-Custom-Types

Problem-Statement: Primitive types (e.g. u32, &str, &[u8]) allow invalid states to travel across function boundaries (Q: to exist inside functions?). Checking preconditions on raw primitives at every function boundary is error-prone, possible (easy) to overlook or forget, and fails to diagnose where issues/corruption occurred (e.g. logical bugs, bitflips, memory issues, etc.).

Rule 5 Solution: Enforce "Parse, Don't Validate" using Newtypes, Non-Zero primitives (NonZeroU32), and bounded enums. A constructor validates untrusted input once and packages it into a verified type with private fields. Internal functions only accept and return these sound types. If an invariant within a custom-type is violated later (e.g., bitflip/memory corruption), the internal requirement-check catches it at the point of use as a distinct error code rather than causing silent corruption or undefined state.



Three-Level Recovery Hierarchy ("Let It Fail & Recover")
Every production-release failure resolves into one of three bounded recovery tiers:
- Tier 1: Micro-Retry (Local) — If err.is_retryable() is true, the immediate caller repeats the bounded operation (with backoff/sleep).
- Tier 2: Step Fallback / Safe Degradation (Subsystem) — If non-retryable or attempts are exhausted, the subsystem safely aborts the current command, resets its internal state to a known-safe baseline, drops invalid work, and reports the error code up the stack.
- Tier 3: Macro Re-initialization (Supervisory) — If an unrecoverable invariant/hardware failure occurs, the outer supervisor loop flushes queues, reinitializes the module/device, and continues execution without taking down the entire runtime.

# Modes and Case-Handling in General

### What are the questions?

The overall intention here is to try to framework the overall problem-spaces around what I will call "modes" of processing and compiling code 
look back over time at what has been done and discussed and to explore and 

Each language, and future, each project in each language, may (and likely should) have different answers, but there may be value in having a somewhat standard set of starting questions that:
A. People (individuals and teams) doing a project can use as a planning guide as a communication aid. 

B. People can use to aid a somewhat interdisciplinary discussion across projects and languages, domains, etc.


Maybe Universal Questions:
1. How are you managing Test-Mode?
2. How are you managing Debug-Mode?
3. How are you managing production-release mode?
4. Are there (other) modes that this project needs (or does not need)?
5. How are you using terminology around 'modes' and 'cases, errors, handling,' and 'recovery'?
6. How do you recover state in the event of "errors" (e.g. 4 levels)?
7. Stack, Heap, Memory: Are there rules or priorities around using heap and stack memory?
8. Threads, Concurrency, Atomics: 
9. Error-tracability: What are your needs and plans for tracing errors?
10. Risk, Priorities & Scope: Are there concerns such as security or data privacy? (e.g. in terms of how cases/errors & memory are handled)
11. How maintainable for how long in the future is this project intended for? (e.g. From 'This afternoon, used once ever' to 'Many decades')
12. Are there language-features that you want to preclude from use for this project (e.g. as part of the above answers)?
13. What are the pros and cons of different languages and approaches in the context of the above answers?

Where a detailed set of coherent answers to the above questions may constitute a 'framework': The goal of the framework described below is not an attempt to create a one-sized-fits-all meta-uber-TOE-framework, but rather to explore one example of a (hopefully close-ish to) coherent approach expressed in one language (Rust).

Hopefully in future, as more such frameworks are compared, it will become clearer what general-universal questions and aspects there are, if there are common domain specific variations, and which areas are too highly domain specific to try to speak about in general terms. Progress on this is a goal.


#### 'Power of 10' Style Modes and Case-Handling in Rust:
- Bullet Point Outline Summary
- More Detailed Discussion



# Compile-Mode-Differentiated Case & Error Handling Policy & Framework: A Three-Mode System using Fieldless Error-Codes

Setting aside issues with terminology,  and the details of specific programming languages, the 2006 'Power of 10' paper sets out a coherent argument for a general need to manage things such as errors across at least universal modes of running and testing code: A real-world-use mode, a debugging mode, and a testing mode. 

In a general and abstract way this could set out something like this:

1. Production (real world use): The satellite can't crash, every issue must be handled smoothly, and no security-issues can be allowed (including no sensitive data leaked in 'error' messages).

2. Debugging should be close to Production, but during errors as much diagnostic data as possible should be reported (no privacy here).

3. Testing makes use of deliberately crashing the program to get deep stack trace granularity about what went wrong, and to detect specific issues.

Since NASA has articulated this, it should be common sense to try to follow these concepts. To do this there are at least two classes of challenges:

1. There are notable ambiguities of terminology (including in the 2006 paper) that need to be untangled and clarified so that a detailed plan is unambiguous.

2. In each language (c, Rust, Zig, etc.) there is a context of what can and should be done and how within that language (and often unique jargon for that language).

This framework attempts to clarify the terminology and to present a preliminary model for how case-handling per mode can be done using Rust. These goals can be laid out in more detail in a page or two of text (not too long), but, as you will see, it is often not trivial to follow these (common sense) rules within the grammar of a language. This raises questions: As in the spirit of 'Nudge' (See Thaler & Kahneman), this mysterious gauntlet of challenges makes it less likely that most software will be robust. Language design should nudge people in better default directions, not worse directions and outcomes.

As a note, while the main focus here is a framework for Rust, having an ever-deeper understanding of how case-handling works in different languages, such as Zig, is likely important for ongoing improvements in the area of mode management and case handling.



## Terminology Problems

One of many terminology problems here is around the jargon for 'production' 'release' builds of code and products. In the wider world 'production' does not have the same meaning that it has in IT and software development. In film, for example, 'in production' means the film is still being shot, and 'post-production' refers to work after shooting scenes and before the film is finished to show in theaters (such as final editing, effects, etc.). For software, web-development, data-science, and cloud-deployment (such as AWS built cloud infrastructure), 'production' is the conventional jargon term meaning 'in real-world use' where there are often 'development,' 'staging,' and 'production,' versions of the system. 'Production' software is the type used by real users, live in the real world. The phrase 'in production' is common to refer to either the production tech-environment or to 'production' meaning real-world-use, or both. (It not clear who decided to describe software that is finished and being used in the real world as 'still in production,' but as the saying going in computer science 'naming things is hard.')

Rust, and likely other languages, have a similar range of phases (and phrases), such as testing with cargo-tests, debug-builds, and there is another jargon-term "release" that is likely comparable to 'production.' These terms and modes are built into the Rust-cargo system by default. You can make your own custom modes too, but those three are always there.

While not ideal, as the average person on the street would have no idea what 'production-release' is supposed to mean, I will try to consistently use the term production-release to refer to the area of final for-real-world-use code/software and product. This hopefully bridges the standard software 'production' jargon and the Rust 'release' jargon, and avoids (as if often difficult in computer science) the problem of alternating between terms (which likely confuses various readers in different ways). The terms "in production" and "release" still exist on their own in the world (the rust term is 'release' not 'Production-release,' developers say 'stage and prod' not 'stage and prod-release,' a 'production environment' is not called a 'production-release environment'), and so these terms may be used in those contexts in this paper. But an attempt is made to consistently refer to production-release code and production-release mode in a consistent way here.


## Three goals (in priority order):
1. Production-release never panics, never halts, never leaks data: Handle and move on.
2. Minimal memory-use in production: no heap in production-release error paths, no error strings compiled into production-release binaries.
3. Ideally: Every error is traceable to one exact location in the code.


There is a lot of room where different projects will go in different directions, such as if and how logging is done, if and how heap-memory is used, and innumerable other case-by-case decisions. The focus here will be the above three goals.

"Static-code-analysis" and avoiding hidden misbehaviors are arguably within the overall message of the 2006 paper and are reasonably generally applicable as sound advice. Memory-use and error-tracing are not strictly necessarily in scope for all programs to be safe and not crash, but the goal here is to try to architect a strict framework for default robustness. Future maintainability is another aspect of this, by being strict we can allow future additions to a project to have a greater chance of being just as sound as the original (instead retractively realizing that the changes broke things in ways that no one ever can ever fully untangle).



## 1. The Three Modes of Output & Case/Error Handling
- Test Mode
- Debug Mode
- Production-Release Mode

Per function: The same function source-code compiles into three behaviors. The function signature and the returned error VALUE are byte-identical in all three builds. The difference is in how the function behaves (per mode) when an error occurs:
- In Test Mode, it can/should crash/panic to produce a detailed stack trace.
- In Debug Mode, it should print or log extensive diagnostic data (and use heap).
- In Production-Release Mode, it must handle cases smoothly (sometimes 'silently'), without crashing, or leaking data (with no heap).


| Mode | Build flag | Panics? | Heap? | Output |
|---|---|---|---|---|
| **Test** (`cargo test`) | `#[cfg(test)]` | yes — `assert!` **in the test function only** | allowed | verbose; asserts on returned error values |
| **Debug** (debug build, not test) | #[cfg(all(debug_assertions, not(test)))] | invariant: yes (debug_assert!); input-validation: no | allowed | invariant: panic + message; input-validation: eprintln! + error returned |
| **Production-release** (release) | always-compiled path | **never** | **never** | terse 2-byte error code returned to caller; no terminal print |

Note: This framework assumes there is no additional 'shoot-yourself-in-the-foot' action, such as turning debug-compilation (or test code compilation) on in your production-release.(!) As Steve Gibson of GRC says: "Just because you can does not mean that you should."
```rust
// Please do not do this:
[profile.release]
debug-assertions = true
```


### Rules for Modes

1. `debug_assert!` in function bodies must be "gated" using `#[cfg(all(debug_assertions, not(test)))]` so the code does not run in other modes. Debug code by default DOES run in test-mode.

2. Verbose diagnostics at a failure site are `eprintln!` gated `#[cfg(debug_assertions)]`. They may use heap (`format!`, `.to_string()`) freely — they are excluded from production-release code.

3. The production-release path (the `if`-check and the `return Err(...)`) is always compiled, in every build. It is the only path that is used in production-release for real-world use.

4. Test-mode code must be gated with #[cfg(test)] and be careful using test-mode tests to debug inside functions. As a rule of thumb, do not use test-mode tests inside functions in stable code, ensuring that all code is testable.

Because production mode is all smooth-cases (effectively silently handling what would be errors), to clearly identify and trace 'errors' test-mode or debug-mode inspection must be used. Values inside a function can only be tested inside a function (i.e. using #[cfg(test)] in the function), in other words: cargo-test is not able to perform tests on specific values at specific places within a function. Only use test-mode inside a function when the desired result of the test is a panic-stack-trace, as this will prevent the rest of the program from running. The risk of stopping the program by using a test-mode test inside a function will prevent later parts of that function from  running. So a cargo-test in a function should be considered a short-term testing-debugging tool (e.g. if you need that stack trace to fix the code, then that is a tool in your toolbox to use), but once the code is stable then (unless in exceptional circumstances) test-mode tests inside functions should be avoided so that the entirety of code is testable and there is no 'untested code' region that was behind another test.


### Disambiguation & Vocabulary 

The original 2006 'Power of 10' rules contain a confusing use of the term "assert" regarding a number of critical valid points vital to the guidance in the paper. 

The 2006 'Power of 10' rules use the single word "assert" when describing three importantly-differing meanings and situations (without providing a way to specify the differences):

1. The assert() macro, which halts the program on failure;
2. The general verb "assert a condition" — to check that something must hold, independent of what happens on failure;
3. A check that, on failure, takes a recovery action (e.g. returns an error) — the opposite of halting.

The 2006 power-of-10 rules demand all three at once: 
1. Tight limits on conditional compilation — even though the standard assert is implemented by the preprocessor and DOES halt (case 1)
2. At least two "assertions" per function (case 2)
3. Never Halting in Production-release: Assertions that take a recovery action rather than halt (case 3)

Such terminology can lead to confusion (e.g. where you are both required to "assert" and prohibited from "assert"ing, because the term "assert" does not have one specific & consistent meaning), especially as there are no clarifying annotations for any use (let along all uses) of the term in the original 2006 power-of-10 paper. The spirit of the paper is clear, but the details become a quagmire.

The term "assert" cannot easily be used precisely here, so this framework aims to consistently use three descriptive terms instead:

1. **required-condition**: the rule itself (e.g. `len <= 32`), independent of build.
2. **requirement-check**: the code that tests a required-condition (the always-compiled `if`).
3. **reaction-to-check**: what happens upon failure, which varies by mode:
  test → panic in the test; debug → panic / verbose eprintln; production-release → terse code returned (possibly a retry-first) (never panic, never halt, never stop: the satellite must not fall out of the sky).

Similarly, this framework also aims to clarify the semantics around "errors" in the same context, where technically an "error" does not refer to all required ways of handling a ~case (in this context), and where there are three importantly distinct modes for how 'case-handling must (or must-not) be done. We need to clarify:

1. the build/compilation mode (production-release / debug / test), and
2. the difference between a case (any condition that must be handled) and an error (a case that is handled by returning Err).

Unfortunately there is no single common-language and technical-term that covers 'error-like-things' across all three modes that we need to manage: test-mode, debug-mode, production-release-mode. The somewhat bulky phrase 'cases that need to be handled' may be the closest we can come, though even that lacks an emphasis on erroneous or problematic-case handling (vs. every part of code that "handles" some logic in a neutral sense). In casual language the word "error" may be the simplest and most accurate term, but as with the term "assert", the word 'error' has been given specific technical meanings that make planning and communication fraught. Which meaning of 'assert' or 'error' is someone using in every instance? Colliding-terms make confusion unavoidable.

Not every "handled case" is technically an "error." The same case may be silently ignored, re-tried, or (in test mode) turned into a 'panic.' Keeping "case" and "error" distinct is a critical part of the source-code management that is implicit in the 2006 power-of-10 guidance, managing specifically different per-mode behaviors in the same 'cases.' We should strive to make more of the implicit wisdom of the spirit of the power-of-10 more explicit and robust for systems-programming and software education more broadly. There is a low probability of all readers of the original 2006 document following a uniform framework with a shared disambiguated lexicon.

## Defensive Programming: "expected" issues vs. "should-not-happen-check"

Another area that can be confusing is 'errors' or 'cases' that logically should-not happen but that in the real world (often) do happen. For whatever reason there is no standard terminology for this concept, and so important defensive-programming checks often appear to be silly mistakes made by a developer (why check for something that in a perfect world cannot happen?) and are then removed by later developers (and then the software crashes 'in production'). 

A solution that hopefully covers this is to standardize two
types of checks. We have "input-validation" that covers the range of 'expected-type'
problems (problems that we know are possible), such as a value being outside of an acceptable range. 
And we have the "should-not-happen-check" for so-called "unexpected" problems (problems that are impossible in an ideal-fictional-world).

Another reason to define these two is to standardize where it may be usually best
to do each type of check.


### Input Validation ("expected") vs. Internal Invariant ("should-not-happen-check")

- **Input validation** (e.g. bad user input, missing file, oversized argument) :
  an EXPECTED case. Requirement-check + terse return
  only, plus debug eprintln.
  Default debugging for input-validation should be non-fatal (eprintln + return) to provide debug-data without panic-halting. 
  Case by case, if a panic-backtrace provides the data needed then panic-assert (for debug) is an option.
  
- **Internal invariant** (a value that in a perfect-world 'cannot happen' but that in real life can be the result of a bitflip, hardware failure, adversarial attack, undefined behavior, or other corruption): all three modes apply — debug_assert, test assertion on the
  returned value, and the always-compiled production-release catch. a defensive should-not-happen-check

#### Classification heuristic to decide which pattern to use:
Ask the question "Can this condition fail when the code is correct and memory is sound?"
- Yes -> Input validation ("expected" type)
- No -> Internal invariant ("should-not-happen-check")



## 2. The Error Type: One Fieldless Enum

The project error type is a single fieldless `enum` with explicit e.g. `u16`
discriminants. The discriminant IS the error-code.

#### Note on size choice:
- A tiny project might use u8 (fewer than 256 codes).
- u16 is probably the most common: 65,536 codes
- An extra-big project might need u32 or larger.
- u32 would also allow more sub-divisions or large-blocks of codes, e.g. if you wanted regular block sizes and little risk of running out. 2**16 being 65,536 looks inexhaustible, but if your code-base grew to have 200 types of errors that you wanted to separate into blocks, then that caps a regular block size at 327, which looks less inexhaustible.


#### Single, Central YourProjectError Enum Across Module-Files
The ABCProjectError enum is defined once (e.g., in one .rs module file) and is 
imported wherever errors need to be returned or handled. All modules use the 
same enum variants, so error codes are globally unique and consistent.


```rust
// Debug printing (variant names as text) is compiled in for TEST and DEBUG
// builds only. In a production-release build no Debug impl exists, so no
// variant-name strings are baked into the binary — and any stray `{:?}`
// on a YourProjectError in production-release code fails to compile (intended guard).
#[cfg_attr(any(test, debug_assertions), derive(Debug))]

// Always available, in every build:
// Copy/Clone — the error is a trivially-copyable 2-byte value (no heap).
// PartialEq/Eq — lets callers and tests compare codes (`err == YourProjectError::ThisCondition`,
// `assert_eq!`, `matches!`).
#[derive(Copy, Clone, PartialEq, Eq)]

// Fixes the in-memory representation to a u16 so the discriminant IS the
// 2-byte error-code, making `self as u16` well-defined and the size exactly
// 2 bytes.
#[repr(u16)]
pub enum YourProjectError {
    // io categories (mapped from io::ErrorKind at failure sites)
    IoInterrupted = 1,          // retryable
    IoWouldBlock  = 2,          // retryable
    IoTimedOut    = 3,          // retryable
    IoNotFound         = 50,
    IoPermissionDenied = 51,
    IoOther            = 99,

    // one block of codes per file/struct/feature — see Error-Code-Table rules below
    Fs32tFromStrInputTooLong   = 101,
    Fs32tAsStrLenExceedsBuffer = 102,
    Fs32tAsStrInvalidUtf8      = 103,

    RetryMaxAttemptsZero = 200,
}

impl YourProjectError {
    /// The terse numeric error-code. Available in ALL builds. No heap.
    pub fn code(self) -> u16 { self as u16 }
}

pub type Result<T> = std::result::Result<T, YourProjectError>;
```

### Useful Properties of Fieldless Enums (for this use-context)

- `Copy`, 2 bytes. No heap anywhere in the error path.
- NO `String` payloads. NO `io::Error` payloads (`io::Error` is not `Copy`
  and can itself hold a heap string containing paths/PII).
- The compiler enforces both name uniqueness and discriminant-value uniqueness, so no two variants can collide on a name or a code. (See https://doc.rust-lang.org/error_codes/E0081.html)
- Callers can `match` exhaustively.
- Variant payloads are forbidden. If a caller needs a fact (a length, a
  position), the fact justifies a distinct variant, not a payload.
  Payloads are the leak vector this policy exists to close.


### Error-Code-Table rules (tip: show in enum's doc comment)

1. **APPEND-ONLY.** A code number is never reused or renumbered, ever.
   A code in a years-old field log must still mean one thing.
2. A file/struct/feature can get a reserved hundred-block
   (e.g. `100–199 FixedSize32Timestamp`). Blocks are listed in the enum doc.
3. Variant names are `AcronymFunctionCondition` (e.g.
   `Fs32tFromStrInputTooLong`) — unique, traceable, greppable.
4. The number to meaning table lives in the enum's doc comment. It is
   DOCUMENTATION, not shipped binary data.

### Display: for debug builds only

```rust
/// Human-readable text — DEBUG/TEST BUILDS **ONLY**. Never ships.
#[cfg(debug_assertions)]
impl std::fmt::Display for YourProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            YourProjectError::Fs32tFromStrInputTooLong => "FS32T-101 from_str: input too long",
            // ... one arm per variant ...
        };
        write!(f, "{}", text)
    }
}
```

Implicit in the power-of-10 goals for production-release software is an interesting unique-per-project design question: for software that never stops, halts, panics, etc., how is 'error' information reported? If there is a UI or terminal display, the information may be printed to terminal (without heap) or shown in an info-bar. Or the error-data may be logged or saved to a database. Also, information about the error or 'case-handled' should be as terse as possible, this to manage memory (and avoid heap) but also to prevent any exfiltration of potentially sensitive, private, or security-related data. 

Production-Release binaries can therefore safely contain no error strings. 
A production-release-build log or an info-bar shown to the user can show the error-code-number (e.g. `101`). The number can be looked-up in the Error-Code-Table. 





#### Render the number without heap:

```rust
/// u16 → decimal ASCII in a stack buffer. No heap. Panic-free.
fn u16_to_decimal(mut value: u16, buf: &mut [u8; 5]) -> &str {
    let mut start = buf.len();
    if value == 0 { start -= 1; buf[start] = b'0'; }
    else {
        while value > 0 && start > 0 {
            start -= 1;
            buf[start] = b'0' + (value % 10) as u8;
            value /= 10;
        }
    }
    std::str::from_utf8(&buf[start..]).unwrap_or("?")
}
```


### Default Error-Code Rendering responsibility: The boundary renders. Functions return the error-code but do not render. 

Error-producing functions return the error-code, they never print, never log, never touch an 'info bar' in production-release code. Converting a code to human-facing output (u16_to_decimal to 'info bar,' or the log writer) happens at exactly one place per output surface: the top-level handler that owns state, and the logging facility. Rationale: one rendering site per surface means one place to audit for PII and one place to change formatting.

Note: The default case/error-as-code is designed to be most minimal and safest, but that does not prevent production-release-mode uses of that code. It is entirely possible to have a safe-messaging system that would log or print the error-code and a safe explanatory string. E.g. "Contact tech support" or "Irregular input from Device-E detected" or "Check battery" or "Power supply issue." It is safest to leave the error-code as is, but with care it is possible to be both memory thrifty-enough (even no-heap) and safe-enough. Many messages may be useful to send out and be deemed not a security risk. 

Still when reporting a per-code message (in production-release mode), it is recommended that this is done by another function that would be part of how that 'case' is smoothly handled in production, not by the function that returns the error-result-code.

## 3. Retry-or-Permanent Is Defined Per Error-Code (not number-ranges)

Transience (whether an error is retryable) is a property of the error code, 
defined using a single explicit method (for a single source of truth). 
Behavior is defined in one place, making it easy to audit and update.

The is_retryable() method is required to exhaustively list which errors are 
"transient" (able to be retried). 
- Avoids silent misclassification.
- Requires developers to decide for each error whether it is retryable.


```rust
impl YourProjectError {
    /// Transient failure worth retrying?
    /// EXPLICIT list. Deliberately NOT inferred from number ranges:
    /// range rules ("codes < 50 are retryable") silently mis-classify
    /// appended codes and nothing fails to compile. Numbers are identity
    /// only; behavior lives here, in readable code.
    pub fn is_retryable(self) -> bool {
        match self {
            YourProjectError::IoInterrupted
            | YourProjectError::IoWouldBlock
            | YourProjectError::IoTimedOut => true,

            YourProjectError::IoNotFound
            | YourProjectError::IoPermissionDenied
            | YourProjectError::IoOther
            | YourProjectError::Fs32tFromStrInputTooLong
            | ... => false,
            // NO wildcard -> adding a variant is 
            // a compile error until classified
    }
}
}
```

Retry loops branch on the output of is_retryable() (bounded loop, production-release catch for bad arguments):

```rust
fn run_with_retry(max_attempts: u32) -> Result<T> {
    // Requirement-check: max_attempts >= 1.
    // INPUT VALIDATION — produces code 200, never panics.
    // This guard guarantees the loop runs at least once, so `attempt >= max_attempts`
    // cannot be spuriously true on the first iteration.
    if max_attempts == 0 {
        #[cfg(debug_assertions)]
        eprintln!("RETRY-200: run_with_retry: max_attempts == 0");
        return Err(YourProjectError::RetryMaxAttemptsZero);
    }

    let mut attempt = 1;
    loop {
        match operation() {
            Ok(v) => return Ok(v),
            Err(e) if attempt >= max_attempts => return Err(e), // exhausted: return last error
            Err(e) if e.is_retryable() => { /* sleep / backoff */ attempt += 1; }
            Err(e) => return Err(e),                            // permanent: fail fast
        }
    }
}
```

### The io boundary rule

`io::Error` is converted to a terse code at the failure site. 
`io::Error` (and any path/PII text inside it) is DROPPED at the failure site. 
Only `ErrorKind` is kept, as a discriminant.

Two rules for mapping:

1. Retry-relevant kinds get distinct codes. IoOther is non-retryable.

2. `IoOther` is an explicit policy bucket, not a catch-all
   `IoOther` means "no distinct code assigned, and **non-retryable**." 
   Mapping a kind to `IoOther` = no retrying. 
   If sometime should be retried, it needs its own code.

`io::ErrorKind` is `#[non_exhaustive]`, so the wildcard arm (`_ => IoOther`) is
**required** by the compiler — new kinds can appear in future std versions.
This means you cannot rely on the compiler to force classification of a *newly
added std kind* (unlike your own enum, where the no-wildcard `match` in
`is_retryable` does force it). E.g. Re-audit mapping when updating Rust version.

```rust
impl YourProjectError {
    /// io::Error → terse code. The io::Error (and any path/PII text inside
    /// it) is DROPPED here; only the kind survives as a discriminant.
    ///
    /// Classification note: kinds mapped to `IoOther` are treated as
    /// NON-RETRYABLE by policy (see `is_retryable`). Kinds that should be
    /// retried MUST get their own code, not `IoOther`.
    pub fn from_io_kind(kind: io::ErrorKind) -> YourProjectError {
        match kind {
            // --- transient / retryable: distinct codes required ---
            io::ErrorKind::Interrupted       => YourProjectError::IoInterrupted,
            io::ErrorKind::WouldBlock        => YourProjectError::IoWouldBlock,
            io::ErrorKind::TimedOut          => YourProjectError::IoTimedOut,
            io::ErrorKind::ConnectionReset   => YourProjectError::IoConnectionReset,
            io::ErrorKind::ConnectionAborted => YourProjectError::IoConnectionAborted,
            io::ErrorKind::BrokenPipe        => YourProjectError::IoBrokenPipe,

            // --- permanent: distinct codes for common cases ---
            io::ErrorKind::NotFound          => YourProjectError::IoNotFound,
            io::ErrorKind::PermissionDenied  => YourProjectError::IoPermissionDenied,
            io::ErrorKind::AlreadyExists     => YourProjectError::IoAlreadyExists,
            io::ErrorKind::InvalidInput      => YourProjectError::IoInvalidInput,
            io::ErrorKind::InvalidData       => YourProjectError::IoInvalidData,
            io::ErrorKind::UnexpectedEof     => YourProjectError::IoUnexpectedEof,

            // --- explicit policy bucket: NO distinct code, NON-RETRYABLE ---
            // Required wildcard (io::ErrorKind is #[non_exhaustive]).
            // Everything here is a deliberate "treat as permanent" decision.
            _                                => YourProjectError::IoOther,
        }
    }
}
```

#### Arbitrary examples of Corresponding additions to the enum's io block:
(Append new numbers, do not renumber.)

```rust
    // io categories (mapped from io::ErrorKind at failure sites)
    IoInterrupted      = 1,   // retryable
    IoWouldBlock       = 2,   // retryable
    IoTimedOut         = 3,   // retryable
    IoConnectionReset  = 4,   // retryable
    IoConnectionAborted = 5,  // retryable
    IoBrokenPipe       = 6,   // retryable

    IoNotFound         = 50,
    IoPermissionDenied = 51,
    IoAlreadyExists    = 52,
    IoInvalidInput     = 53,
    IoInvalidData      = 54,
    IoUnexpectedEof    = 55,
    IoOther            = 99,   // explicit policy bucket: non-retryable by rule
```

Matching arms in `is_retryable`:
(no-wildcard `match` forces these to be classified)

```rust
    pub fn is_retryable(self) -> bool {
        match self {
            YourProjectError::IoInterrupted
            | YourProjectError::IoWouldBlock
            | YourProjectError::IoTimedOut
            | YourProjectError::IoConnectionReset
            | YourProjectError::IoConnectionAborted
            | YourProjectError::IoBrokenPipe => true,

            YourProjectError::IoNotFound
            | YourProjectError::IoPermissionDenied
            | YourProjectError::IoAlreadyExists
            | YourProjectError::IoInvalidInput
            | YourProjectError::IoInvalidData
            | YourProjectError::IoUnexpectedEof
            | YourProjectError::IoOther
            /* | ...remaining non-io variants... */ => false,
            // NO wildcard -> adding a variant is a compile error until classified.
        }
    }
```

### Notes:
- Retry classifications above are arbitrary examples, not framework-rules
- `io::ErrorKind` is `#[non_exhaustive]`,  it needs the human re-audit


#### More Disambiguation: "Error Site" & "Catch"
Naming things is hard, so "error site" frequently has one 
(hopefully just one) of two different meanings:
1. The file, function, or line of code where an issue, failure or bug occurred.
2. The place in the code where an issue is detected (and responded-to / handled etc.). 


A detection point, as meant here, is the place in a function where a case is detected and handled, for example where the code decides "This is wrong, so we should return error code {N}." 

The commonly used term "Catch" is sometimes roughly equivalent in spirit to how the term 'detect' is used here, but details vary.

JS/TS, Java, and c# use the common 'Try / Catch / finally' system.
cpp and php use only 'Try / Catch'

Zig uses the terms 'try' and 'catch' but the meanings are different:
- zig try: immediately propagate any error up the stack (return that error)
```zig
const value = try mightFail(false);
```

- zig catch can work in a few different ways:
1. "catch" an error to replace it with a specified default value.
If the function-call works, great! 
If the function call fails, then we should do this other thing instead.
```zig
const a: u32 = addTwenty(42) catch 22;␤
```
2. "catch" the error to run a block of code when that happens.
const x = mightFail(true) catch |err| {
// run this code block if it fails
}

As with the word 'assert,' it is important to note collisions between one or more jargon and common-use meanings.


## 4. Two Standardized Patterns for Detection 

We will define two patterns, because there are two ways that code 
can detect an issue (such as an 'error' or a 'case to handle'): 

Both patterns cover the three-modes of behavior (test / debug / production-release).
Consistently using these two patterns should help reviews, audits, and readability.

1. The If-Detection-Pattern: 
Use an if statement where you can ask as an 'if' question:
E.g. "is this length ≤ 32?" or "is this denominator zero?"


2. The Match-A-Function-Call-Pattern
Use a 'match' for functions that can fail (and, coding defensively, 
all functions in this system should return a result).
E.g.  Some function you called told you that it failed. 
You called a function such as from_utf8 or opened a file 
and the check happened inside that function. 
The function you called handed you a 'success-or-failure' result. 


### Avoid '?' if Obscures error
Use of '?' can be in tension or conflict with at least two of this framework's  goals / mechanisms (per-site variants, mode-gated diagnostics). Using '?' might reintroduce untraceable, unaudited propagation. In other cases not using '?' might create redundant error-data not directed at the original source if that can be propagated. Real life tends to be full of strange cases. 
- '?' should be avoided where it would obscure an error.

Matching each call with a clear error pointing to that case improves transparency and communication.

 
### The If-Detection-Pattern 
This pattern is for checks that are a condition written as an if statement. 
e.g. ("is X length ≤ 42?", "is Y denominator 0?")

// --- Check: <the required condition, in words> ---
// <input validation | internal invariant> — <one line: why this check exists>

// Internal invariants ONLY (omit this line for input validation):
// panics in debug builds; compiled out of test and production-release builds.
#[cfg(all(debug_assertions, not(test)))]
debug_assert!(condition, "ACRO-NNN: fn_name: detail {}", value);

if !condition {
    // Debug/test diagnostic is permitted to format text (heap OK) 
    // because this line
    // is compiled out of production-release builds entirely.
    // Note: single source of truth for number must be enforced,
    // no hard-coding of code-numbers in messages
    #[cfg(debug_assertions)]
    eprintln!("ACRO-{}: fn_name: detail: {}", YourProjectError::AcroFnCondition.code(), value);

    // Compiled in EVERY build; the only response that exists in
    // production-release: return the 2-byte code. No print, no heap.
    return Err(YourProjectError::AcroFnCondition);
}

Every detection-site gets a cargo test asserting the returned value:

```rust
#[test]
fn acro_fn_rejects_condition_with_code_nnn() {
    let err = the_function(bad_input).unwrap_err();
    assert_eq!(err, YourProjectError::AcroFnCondition);
    assert_eq!(err.code(), NNN);
}
```

Tests for a function's detection sites should live in the same file as the function,
in that file's `#[cfg(test)] mod <filename>_tests` block, so the
Error-Code-Table doc-comment, the detection sites, and their tests are auditable. 

Test names should follow the form
`<acronym>_<fn>_<condition>_with_code_<nnn>`
so that failures can be more easily and clearly traced.


### Testing invariant catch paths (unreachable via the public API)

A production-release catch for an internal invariant is normally unreachable
through public constructors — that is what makes it an invariant. Its
test must construct the invalid state DIRECTLY, which is possible
because tests live in the same file/module and can access
private fields:

```rust
#[test]
fn fs32t_as_str_catches_corrupt_len_with_code_102() {
    // Construct an invalid value directly (same-module private access).
    // This simulates post-construction corruption (e.g. bitflip).
    let corrupt = FixedSize32Timestamp { data: [0u8; 32], len: 99 };
    let err = corrupt.as_str().unwrap_err();
    assert_eq!(err, YourProjectError::Fs32tAsStrLenExceedsBuffer);
    assert_eq!(err.code(), 102);
}
```

Do not add public test-only constructors or pub(crate) fields to enable other tests, that would widen the API surface that the invariant protects.


### The Match-A-Function-Call-Pattern: failure detected via a returned `Result` (not a testable bool)

When the required-condition is checked BY a fallible call (e.g.
`std::str::from_utf8`, a parse, a syscall wrapper), the requirement-check
is the `match` on that call's result (no requirement to test the condition
separately with a duplicate check).

#### Two different match-handling error values:

The Match-A-Function-Call-Pattern involves two different error values of different kinds. This is important for clear handling, for naming and communication, and (if relevant) for heap and memory management.


#### Match-error type 1: The error-code for this system.
This is more clearly no-heap, slim, and predictable. 
In every build-mode, only this 2-byte error-code leaves the function.


#### Match-error type 2: The callee's error, which depends on what function is called. Hopefully it is usually standard a small fixed-size struct the fallible function returns on failure. 
E.g. the Utf8Error is two integers, and stack-only)
```rust
struct Utf8Error {
    valid_up_to: usize,      // the byte position where the input stopped being valid UTF-8
    error_len: Option<u8>,   // length of the invalid byte sequence
}
```
But this is just one clean example, the details are up to each function.
The most memory-use-defensive we can be in production-release code is to '_details' whatever this error is (note: even if memory-use is not a concern, this error-data might contain privacy or even PII related information that should not be exfiltrated in production-release builds).


The callee-function's error value has whatever type that function declared. Many std errors are small stack-only structs, but other std library, and especially 3rd party, errors may own heap allocated inside the callee.

When using any standard-library or third-party function not built 'natively' within a project, heap-use is likely:
- io::Error can sometimes contain an N-length message (using heap)
- String`, `Box<dyn Error>` & `anyhow::Error` can use heap
- Custom library errors with `String`/`Vec` fields can/will use heap.

This framework does not / cannot prevent a dependency from using heap, 
but it can drop what is returned and allocates nothing further to heap. 


All three modes live in the
`Err` arm:

```rust
match fallible_call(input) {
    Ok(value) => Ok(value),   // or continue using `value`
    Err(_detail) => {
        // Debug Mode: panic in debug builds ONLY (not test, not production-release).
        // `debug_assert!(false, ...)` is the standard way to panic from
        // inside an Err arm; it compiles out of production-release builds.
        #[cfg(all(debug_assertions, not(test)))]
        debug_assert!(false, "ACRO-NNN: fn_name: {}", _detail);

        // Verbose diagnostic, heap OK, compiled out of production-release.
        // For an internal invariant this runs in TEST builds only: in a debug build
        // the debug_assert!(false, …) above panics first, so execution never reaches here.
        #[cfg(debug_assertions)]
        eprintln!("ACRO-NNN: fn_name: {}", _detail);


        // Production-release Mode: terse code. No heap. No PII. No print.
        Err(YourProjectError::AcroFnCondition)
    }
}
```


**Input-validation version** — identical to the above, but with the `debug_assert!(false, …)` line removed, because a failure on untrusted input is expected, not a bug, for where Debug should not panic:

```rust
// INPUT VALIDATION version — for a call that can legitimately fail on
// untrusted input (e.g. from_utf8 on external bytes). Debug must NOT panic.
match fallible_call(input) {
    Ok(value) => Ok(value),
    Err(_detail) => {
        // NO debug_assert!(false, …) here — this failure is expected, not a bug.

        // Verbose diagnostic, heap OK, compiled out of production-release.
        #[cfg(debug_assertions)]
        eprintln!("ACRO-NNN: fn_name: {}", _detail);

        // Production-release Mode: terse code. No heap. No PII. No print.
        Err(YourProjectError::AcroFnCondition)
    }
}
```




## 5. Things Production-Release Code Must Not Do

### Banned in Production-Release Error Paths

- `format!`, `to_string`, `String`, `Box<dyn Error>`, any use of heap
- `io::Error` stored inside the project error type.
- `unwrap` / `expect` / `panic!` / body `assert!`.
- Error text containing paths, file contents, env vars, user data, PII,
  or implementation detail. (Production-Release ships no error text at all.)
- Number-range behavior rules ("codes 1–49 mean X").
- Reusing or renumbering an existing error-code.

### Avoiding Production-Release Panic
(Note, this section is likely not 100% exhaustive.)

These operations also panic with no opt-out (so, in production as well), so they should be avoided or handled correctly when used in case-handling:
let x = buf[i];        // panics if i >= buf.len()   — ALL profiles
let s = &data[a..b];   // panics if a > b or b > len — ALL profiles
let q = num / den;     // panics if den == 0         — ALL profiles
let r = num % den;     // panics if den == 0         — ALL profiles

When indexing, slicing, or dividing in production-release, use non-panicking versions of operations: .get() / .get_mut() instead of [], checked_div / checked_rem instead of / and %, split_at_checked instead of split_at.

#### Plain Arithmetic Operators
Rust's plain arithmetic operators (`+`, `-`, `*`, `<<`, `-x` negation, etc.) have build-profile-dependent behavior that is controlled by `overflow-checks` setting:

| Profile | `overflow-checks` Default | Behavior on Overflow |
|---|---|---|
| dev / debug | `true` | **panics** ("attempt to add with overflow") |
| production-release | `false` | **silently wraps** (two's-complement wraparound) | 

With default settings, using plain arithmetic operators:
- In debug build, `255u8 + 1` panics.
- In production-release build, `255u8 + 1` evaluates to `0` and execution continues with a wrong value.

So: use checked_add / checked_sub / checked_mul / checked_shl in production.



### Appendix 1: Links & References
- https://en.wikipedia.org/wiki/The_Power_of_10:_Rules_for_Developing_Safety-Critical_Code 
- https://spinroot.com/gerard/pdf/P10.pdf 
- https://spinroot.com/static/index.html 
- https://web.eecs.umich.edu/~imarkov/10rules.pdf 
- https://www.stroustrup.com/JSF-AV-rules.pdf 
- https://www.youtube.com/watch?v=JWKadu0ks20 
- https://en.wikipedia.org/wiki/Static_program_analysis 
- https://www.oreilly.com/library/view/designing-data-intensive-applications/9781491903063/ 
- Mike Acton ("Data-Oriented Design and C++," CppCon 2014)


### Appendix 2: Summary of Revised 10 Areas

We can try to make a list of '10 Rust Production Rules' updated for broader general systems programming in 2026 based on NASA's 2006 'Power of 10' rules that were originally narrowly framed for c for embedded-systems. 

These are ideals to be followed where possible and sensible, not as absolute pedantic rules:

1. no unsafe stuff: 
- no recursion  
- no goto 
- no pointers 
- no preprocessor branching
(Term collision note: Technically an 'unsafe code' block in Rust may be required for cases such as naked/assembly code or to interact with a Posix-OS, as in the case of raw-terminals. While use of jargon-'unsafe' blocks should be avoided where possible, the term 'unsafe' may be a jargon-term that does not mean that a specific best-practice rule was violated.)

2. Loops: either firmly bounded or unbounded:
- Upper bound on all normal-loops (to make sure they do **not** keep looping)
- Failsafe for all always-loops to make sure they **do** keep looping (e.g. additional restart layer)

3. Pre-allocate all memory (no dynamic memory allocation)
- Production code should minimize or eliminate use of heap (e.g. very terse error messages that do not leak any user-data)
- Debug and testing often make sense to use heap and this code is not in production-binaries (e.g. detailed error messages)
- Clearly separate lazy-convention from real-need. With tools such as "Buffy'
github.com/lineality/buffy_stack_format_write_module, it is not necessary to use heap for string formatting.

4. Clear Function Scope and Data Ownership: 
Part of having a function be 'focused' means knowing if the function is in scope. Functions should be neither swiss-army-knife functions that do too many things, nor scope-less micro-functions that may be doing something that should not be done. Many functions should have a narrow focus and a short length, but definition of actual-project scope functionality must be explicit. Replacing one long clear in-scope function with 50 scope-agnostic generic sub-functions with no clear way of telling if they are in scope or how they interact (e.g. hidden indirect recursion) is dangerous. Rust's ownership and borrowing rules focus on Data ownership and hidden dependencies, making it even less appropriate to scatter borrowing and ownership over a spray of microfunctions purely for the ideology of turning every sub-operation into a microfunction just for the sake of doing so. (See more in rule 9.)

5. 'Case Handling' per Mode & Defensive Programming: debug-assert, test-assert, prod safely check & handle, not 'assert!' panic in production

6. Manage ownership and borrowing
- Rust is designed to greatly assist here (vs. c).

7. Manage return values: 
- use null-void return values 
- check non-void-null returns
- see above for designing and checking return values to handle cases of invalid other return-value cases.
- always have functions return a 'result' so errors and cases can be handled

8. Manage conditional compilation: Navigate debugging and testing on the one hand and not-dangerous conditional-compilation on the other hand:
- Here 'conditional compilation' is interpreted as significant changes to the overall 'tree' of operation depending on build settings/conditions, such as using different modules and basal functions. E.g. "GDPR compliance mode compilation"
- Any LLVM type compilation or build-flag will modify compilation details, but not the target tree logic of what the software does (arguably). 
- 2025+ "compilation" and "conditions" cannot be simplistically compared with single-architecture 1970 pdp-11-only C or similar embedded device compilation.

9. Communicate: 
- Use doc strings; use comments. 
- Document use-cases, edge-cases, and policies (These are project specific and cannot be telepathed from generic micro-function code. When a Mars satellite failed because one team used SI-metric units and another team did not, that problem could not have been detected by looking at, and auditing, any individual function in isolation without documentation. Breaking a process into innumerable undocumented micro-functions can make scope and policy impossible to track. To paraphrase Jack Welch: "The most dangerous thing in the world is a flawless operation that should never have been done in the first place.")
- Rather than using '?' for terse function calling, when possible have detailed error handling.
- Rather than having a result hidden in let _ =, allow that result to be shown in debugging

10. Use state-less operations when possible:
- a seemingly invisibly small increase in state often completely destroys projects
- expanding state destroys projects with unmaintainable over-reach


### Appendix 3: Advanced: using error-catching and error-correcting features to recover from accidental or adversarial value-distortions.

1. redundant-checked values (less likely if both copies altered)
2. Hamming code value formats
3. 

### Appendix 4: Advanced: Erasure of sensitive values as part of handling
