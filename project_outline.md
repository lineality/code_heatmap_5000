# HIX Heatmap Indices for Compute (Project Outline)
*"Somebody wakeup HIX."*

### Liebig-Barrel, Physical, Data, Time, Memory, Behavior, Code, Compilation, Compute, Riparian Modeling

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


#### Raw-Type Indices:
1. Count-Heat-Index Per Loop & Function
2. Time-Heat-Index Per Loop & Function
3. Some Count Heat Function(s) Per Loop & Function
4. Inner-Element-Heat within Notable Loops & Functions




### Interpretive Indices: Good vs. Bad Indices
A critical distinction between types of indices is the basic distinction between saying that a loop or function is prominent and should be vs. saying that a given region is prominent and it should not be, indicating that something should be changed. This may be similar to saying highly-represented in a neutral way, vs. something being over-represented in a pejorative way. 

There may also be ~intermediary levels of index and evaluation. (Any of these  may be in highly specific areas with specific data, measures, and tests.) e.g.

1. raw counts and times (possibly together, possibly sometimes necessarily not co-recorded).

2. Areas of interest (e.g. 'hot loop' that should be scrutinized but might already be as good as it gets).

3. Possible Warning Signs of possibly-bad measureables, but still needs manual inspection. 

4. Highly likely bad-behavior (still needs inspection, but unlikely to be ok most of the time (e.g. might be bad but least-worst and unavoidable).

5. Measurably-Bad: Definitely something is wrong.

6. Common-Issue & Known common solution-improvements (still open to inspection and decision)

7. Solutions so clear to problems so clear that the solutions can be (if not ideally) fully automated.

(These 'levels' can also be used, in some cases, to identify and organize report sections.)


### Contextual Tradeoff Priorities for Optimization

In the short term for planning-organization, in the long-term for specific functionality, a user should set down what their Contextual Tradeoff Priorities for Optimization are.

Three dimensions for planning/specifying contexts and priorities:
- 1. Policy x 
- 2. Priority-Focus x 
- 3. Code-Region-Type 

1. Policy: 
- Neutral Policy
- Balance (or Balance with {other area([s])})
- Exclude / Must not use at all 
- Max(imize) / Use as Cheapest 
- Exclusive / Use Exclusively 
- Min(imize) / Reduce-Footprint
- Minimized in Hot Regions


Priority-Focus Areas:
- Security / Privacy
- Resilience, Redundancy, & (Edge) Case Handling
- Stack Memory
- Heap Memory
- Network-Use
- CPU Core Use
- Disk I/O Use
- GPU Use
- Scalability
- Boot-Time
- Shutdown-Time
- Speed / Action-Response-Time
- Database-ACID Time
- Cross-Platform-Ness
- Edge-Embedded-Ness


### Code-Region-Type:
- Hot, 
- Gray-Oatmeal, 
- Cold


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
- MVP 1.6: Specific Implementation-Area Analysis
- MVP 1.7: Region Zoom-In Data-collection & Analysis

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

### Scoping Sources & Targets

This is a larger heterogenous problem space with no single navigation path or context, but we can and will start somewhere.

The question of what data are being targeted and what the options are for getting those data will also be factors in approaches taken. 

For example:

1. Getting and analyzing basic count and time data make sense as a first-scope with more elaborate data and analysis being later scope.

2. The differences between 1. perf 2. Samply, also notable. While linux only, perf seems to align better with the scope of this project, while Samply seems to suffer from the 'fast fashion software curse': Easy! Everywhere! Convenience! Trendy! Don't get left behind! Believe! which are usually red flags; as I understand it Samply closely fits the very standard profile of 'Start fast, move fast, and hit brick wall fast' (and then redesign from square one without that fatal dependency flaw).

If there are notable Samply deep-features (not conveniences) then possible future scope may be optional Samply analysis. E.g. samply use of on-the-fly analysis and 'framehop' (if those are correct to mention) might be used or their concepts borrowed.

On-The-Fly-perf may be a needed partner-module that goes with HIX. On the other hand, being able to record an optional audit-record (however large) may also be useful functionality. 

3. Manual inspection (e.g. adding counting/timing monitoring to code) is probably not needed or desirable for initial overall count-time scope, though it may be useful for specific-region-zoom-in later. LLVM, linux-perf, and cargo, may provide best-matching tools that align with 'first-scan' starting features.

4. Other tools like rad-debugger features should be examined.


#### Future Features

Future functionality goals are also part of the puzzle, for example where first-features are expected to lead-bridge to those (or not). 

Trying to dig or peer in some way to some depth into the 'implimentation' areas mentioned above and explore what situations and possible improvement directions are is the overall scope-goal.

While using existing tools may make the most sense or be unavoidable, the ethos of the type of software the HIX is built with values 'roll your own' 'in-house' zero-dependency type tools. This might be unrealistic or infeasible, but long-term deeper features and maintainability usually interdigitate with native in-house implementations. E.g. one cannot very well compare the resulting binaries and memory-count-time behaviors across different OS, architectures, and compilers, and build-settings, if LLVM is built in as the hidden-magic-kludge. The initial focus here is on Rust, but the larger problem space extends across c-family, fil-c, zig, odin, embedded devices, and may get into the 'Proto-Type' set of puzzles ( https://github.com/lineality/operations_memory_state_manager ). Again, naive-ambission may be impractical, and the target-scope is not (and should not be) all of the real larger problem space, but knowing what that problem-space is is still required due-dilligence. 


### Two Approaches Compared

In a context of First-Scope-Design we will consider two general approaches, of which a hybrid may (or may not) eventually be used. 
1. Using Linux, LLVM & Rust compile tools to inspect without modifying code.
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


# File Structure for Use

Test / run / modeling data should be saved to and accessed from a standardized and clearly named /modeling_data/ directory (which is safely git-ignored)

For specific data-sets, test-sets, runs, etc. within /modeling_data/ :

The sub-folders should (for organization) have unique names. HIX should give you a refuse & error-warning if you try to re-use an existing dir, e.g. potentially overwriting existing files.

The wizard can also help manage this for the cargo, llvm, perf etc. steps that also should mind file-paths.

"Run" & "analysis" names and numbers should assume they exist with past and future "runs" (so 'new!' 'shiny!' 'live in the moment!' are not good names) and should not reuse colliding names. Each name can tersely mention what it analyses: 

- run_001_lines_editor_cut_paste
- run_002_pumpkin_world_spawn_only
- analysis_001_starting_baseline
- analysis_004_after_fix_1


e.g.
```
run_NNN_<scenario> 
analysis_NNN_<purpose> 
```


# MVP-1 Scope:

1. Use perf tools to look at functions and loops, not (yet, if ever) file-changes for bespoke regions within functions and loops.
(Claude 5.5 is forever in a flame war with itself between using cargo/samply or perf... I cannot figure out why. cargo/samply maybe used later or in some other cases maybe)

2. Use/Output a parameter-specified Top-N quadrant report, e.g. overall data about each quadrant, and top-N examples of path-to-fn/loop in 3/4 quadrants.

3. For quad 4:4 (low,low) for which there is no need to give examples (though 
one or two might be academically useful in terms of looking at the bottom N lowest of the low...) the low/low portion of the quadrant could contain some overall metrics such as what portion of function / loops are in that group, maybe what portion of time is used by that group (which might be an indicator of how big the gaps are (e.g. if 99.9% of functions and loops are in low, low, that's probably a good sign... maybe). (Note: maybe there should be a high-level quad for overall data for all four regions?)

4. "You're a wizard, Heatmap!": Make a wizard_cli_module.rs, a Q&A bash-using module for (fedora) linux that can be called by main to semi-automate the overall process (including checking to see that things are installed). E.g. Q&A -> What is the path to the crate you want to test (required)? Is this default path to save the results-files ok (y/n)? [] Is this default path to save the report files ok (y/n)? [] etc. Store files in a standard modeling_data dir. (As the overall process grows, having a clear cli-wizard will be important for practical use, standardized use, and arguably repeat/audit/re-test use. (e.g. possibly having, saving and (re) running (sharing, etc) a wiz_config_{} file (whatever format). 

5. Time/duration of test probably should be included...

6. Choice of and use of perf over samply should be fully explained/documented.

7. No Yard Sale: All run/modeling files should be placed in / accessed from some standard dir such /modeling_data/ that is safely git-ignored

8. Ideally any data-visualization in reports does not use any 3rd party libraries, simple ascii version is highly practical (and terminal friendly).


# Not in MVP-1:
1. super-nuanced async/atomic/concurrent/parallel analysis


# Far Out of Scope (currently):
1. automated fixes or actions on original code itself (suggestions are in scope)
2. replacing or reinventing existing tools / features
3. languages other than Rust
4. a 'heavy AST', type-analysis, etc.
5. extremely arcane/exotic use-cases
6. per-line analysis is not anticipated (that might go along with only within specific regions within functions/loops, which is all future scope)
