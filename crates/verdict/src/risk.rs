//! Lexical NHR risk classifier (SPEC §8). Scans only the changed lines
//! of a unified diff for concurrency / unsafe / floating-point tokens
//! and routes any would-be accept that hits one to needs-human-review.
//! Deliberately lexical and conservative: over-triggering costs a human
//! a look, under-triggering ships an unreviewed risk — so plain token
//! presence decides, no semantic analysis.

const CONCURRENCY: &[&str] = &[
    "pthread_",
    "std::sync",
    "std::thread",
    "Atomic",
    "atomic_",
    "__atomic_",
    "__sync_",
    "Mutex",
    "RwLock",
    "Condvar",
    "mpsc",
    "rayon::",
    "memory_order",
    "volatile",
];

const UNSAFE: &[&str] = &[
    "unsafe ",
    "transmute",
    "from_raw",
    "MaybeUninit",
    "UnsafeCell",
];

/// The spaced tokens carry their own trailing boundary; `f32`/`f64` get
/// hand boundary checks below.
const FP_SPACED: &[&str] = &["float ", "double "];
const FP_TYPES: &[&str] = &["f32", "f64"];

fn is_ident_letter(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// True when `line` contains `token` not glued to an identifier letter on
/// either side. Digits are allowed neighbors so literal suffixes like
/// `1.0f32` still trigger; letter neighbors (identifiers, most hex
/// strings, e.g. `buf32`, `deadf64b`) do not.
fn has_fp_token(line: &str, token: &str) -> bool {
    let bytes = line.as_bytes();
    let mut start = 0;
    while let Some(pos) = line[start..].find(token) {
        let i = start + pos;
        let before_ok = i == 0 || !is_ident_letter(bytes[i - 1]);
        let after_ok = i + token.len() >= bytes.len() || !is_ident_letter(bytes[i + token.len()]);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

fn is_changed_line(line: &str) -> bool {
    (line.starts_with('+') && !line.starts_with("+++"))
        || (line.starts_with('-') && !line.starts_with("---"))
}

/// Classify a unified diff into risk-signal names. `fp_tolerance` is true
/// when the target's equivalence mode is fp-tolerance, itself a §8 signal.
/// Returned names are unique; empty means no signal.
pub fn classify(diff: &str, fp_tolerance: bool) -> Vec<String> {
    let (mut concurrency, mut unsafe_sig, mut fp) = (false, false, false);
    for line in diff.lines().filter(|l| is_changed_line(l)) {
        concurrency = concurrency || CONCURRENCY.iter().any(|t| line.contains(t));
        unsafe_sig = unsafe_sig || UNSAFE.iter().any(|t| line.contains(t));
        fp = fp
            || FP_SPACED.iter().any(|t| line.contains(t))
            || FP_TYPES.iter().any(|t| has_fp_token(line, t));
    }
    let mut signals = Vec::new();
    if concurrency {
        signals.push("concurrency".to_string());
    }
    if unsafe_sig {
        signals.push("unsafe".to_string());
    }
    if fp {
        signals.push("floating-point".to_string());
    }
    if fp_tolerance {
        signals.push("fp-tolerance-equivalence".to_string());
    }
    signals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrency_tokens_trigger() {
        let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n+    let m = Mutex::new(0);\n";
        assert_eq!(classify(diff, false), vec!["concurrency"]);
        let diff = "-    pthread_create(&t, NULL, worker, NULL);\n";
        assert_eq!(classify(diff, false), vec!["concurrency"]);
    }

    #[test]
    fn unsafe_tokens_trigger() {
        let diff = "+    unsafe { ptr.read() }\n";
        assert_eq!(classify(diff, false), vec!["unsafe"]);
        let diff = "+    let v: Vec<u8> = Vec::from_raw_parts(p, n, n);\n";
        assert_eq!(classify(diff, false), vec!["unsafe"]);
    }

    #[test]
    fn floating_point_tokens_trigger_with_boundaries() {
        assert_eq!(
            classify("+    let x: f64 = 1.0;\n", false),
            vec!["floating-point"]
        );
        // Literal suffixes have a digit neighbor and must still trigger.
        assert_eq!(
            classify("+    sum += 1.0f32;\n", false),
            vec!["floating-point"]
        );
        assert_eq!(
            classify("+    double acc = 0.0;\n", false),
            vec!["floating-point"]
        );
        // Glued to identifier letters: not an FP token.
        assert!(classify("+    let buf32 = [0u8; 4];\n", false).is_empty());
        assert!(classify("+    hash = \"deadf64beef\";\n", false).is_empty());
    }

    #[test]
    fn fp_tolerance_flag_is_its_own_signal() {
        assert_eq!(classify("", true), vec!["fp-tolerance-equivalence"]);
        assert_eq!(
            classify("+    let x: f32 = 0.5;\n", true),
            vec!["floating-point", "fp-tolerance-equivalence"]
        );
    }

    #[test]
    fn clean_diff_yields_no_signals() {
        let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n\
                    -    let n = items.len();\n+    let n = count;\n     let done = true;\n";
        assert!(classify(diff, false).is_empty());
    }

    #[test]
    fn only_changed_lines_are_scanned() {
        // Tokens on context lines and in file headers must not trigger.
        let diff = "--- a/src/mutex_wrapper.rs\n+++ b/src/mutex_wrapper.rs\n\
                    @@ -10,4 +10,4 @@\n     let guard = lock.mutex();\n\
                    +    let n = count;\n";
        assert!(classify(diff, false).is_empty());
    }

    #[test]
    fn signals_are_deduped() {
        let diff = "+    let a = Mutex::new(0);\n+    let b = RwLock::new(0);\n\
                    +    unsafe { transmute::<u32, f32>(x) };\n";
        assert_eq!(
            classify(diff, false),
            vec!["concurrency", "unsafe", "floating-point"]
        );
    }
}
