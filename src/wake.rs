//! Deterministic, forgiving check for the wake-up phrase in a local transcript.
//!
//! The transcript comes from a small on-device model, so words can come out slightly
//! wrong ("Júnior", "despiértate", "Junior, wake up."). A transcript wakes Junior when
//! one word is close enough to the name and one word (or two adjacent words, for
//! "wake up") is close enough to a wake word, by edit distance.

/// How Junior's name may be written once normalized.
const NAMES: &[&str] = &["junior"];
/// Wake words, normalized; "wakeup" matches the two words "wake up".
const WAKE_WORDS: &[&str] = &["despierta", "despiertate", "wakeup"];
/// Minimum similarity (1 - edit distance / longer length) for a word to count.
/// Measured on real recordings: wake phrases scored 1.0, other sentences at most 0.64.
pub const MIN_SIMILARITY: f64 = 0.75;

pub fn is_wake_phrase(text: &str) -> bool {
    let words = normalize(text);
    let pairs: Vec<String> = words.windows(2).map(|w| format!("{}{}", w[0], w[1])).collect();

    let has_name = words.iter().any(|w| matches_any(w, NAMES));
    let has_wake_word = words.iter().chain(&pairs).any(|w| matches_any(w, WAKE_WORDS));
    has_name && has_wake_word
}

fn matches_any(word: &str, targets: &[&str]) -> bool {
    targets.iter().any(|t| similarity(word, t) >= MIN_SIMILARITY)
}

/// Lowercase words without accents or punctuation.
fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_alphabetic() => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// 1 - Levenshtein distance / length of the longer word.
fn similarity(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb)));
        }
        prev = cur;
    }
    1.0 - prev[b.len()] as f64 / a.len().max(b.len()).max(1) as f64
}
