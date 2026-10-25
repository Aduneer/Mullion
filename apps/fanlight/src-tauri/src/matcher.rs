//! How well a name matches what was typed. Every word of the query has to match the name, in any
//! order, each the best way it can: the whole name, its start, the start of one of its words, the
//! words' initials ("vsc" for Visual Studio Code), anywhere in it, or its letters in order.

/// What was typed, lowercased and split into words, made once per search.
pub struct Query {
    pub whole: String,
    words: Vec<String>,
}

impl Query {
    pub fn new(text: &str) -> Self {
        let whole = text.trim().to_lowercase();
        let words = whole.split_whitespace().map(str::to_string).collect();
        Query { whole, words }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// The score of `name` (0 to about 1000), or None when it doesn't match.
    pub fn score(&self, name: &str) -> Option<i32> {
        if self.words.is_empty() {
            return None;
        }
        // Cheap test first, without making a lowercase copy, as most names in the file index won't
        // match at all.
        if !self.words.iter().all(|w| {
            let mut it = name.chars().flat_map(char::to_lowercase);
            w.chars().all(|c| it.any(|x| x == c))
        }) {
            return None;
        }
        let lower = name.to_lowercase();
        let shorter = |s: i32| s - ((lower.chars().count() as i32 - self.whole.chars().count() as i32).clamp(0, 60) / 3);
        if lower == self.whole {
            return Some(1000);
        }
        if lower.starts_with(&self.whole) {
            return Some(shorter(900));
        }
        let starts = word_starts(name);
        let mut total = 0;
        for w in &self.words {
            total += word_score(w, &lower, &starts)?;
        }
        Some(shorter(total / self.words.len() as i32))
    }
}

/// One word of the query against the name.
fn word_score(w: &str, lower: &str, starts: &[usize]) -> Option<i32> {
    if starts.iter().any(|&i| lower[i..].starts_with(w)) {
        return Some(if lower.starts_with(w) { 800 } else { 700 });
    }
    if w.chars().count() >= 2 && initials(w, lower, starts) {
        return Some(650);
    }
    if lower.contains(w) {
        return Some(450);
    }
    // Letters in order: a long word for it, or none at all, and fewer gaps is better.
    let n = w.chars().count();
    if n >= 3 {
        let gaps = gaps(w, lower)?;
        return Some((300 - gaps as i32 * 25).max(60));
    }
    None
}

/// How many runs of letters `w` is split into across `s`, less one, matching each letter as
/// early as possible.
fn gaps(w: &str, s: &str) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    let (mut i, mut runs, mut last) = (0, 0, None);
    for c in w.chars() {
        while i < chars.len() && chars[i] != c {
            i += 1;
        }
        if i == chars.len() {
            return None;
        }
        if last.is_none_or(|l: usize| l + 1 != i) {
            runs += 1;
        }
        last = Some(i);
        i += 1;
    }
    Some(runs - 1)
}

/// Whether `w` is made of the starts of consecutive words of the name, each word giving one or
/// more of its first letters: "vsc" or "vscode" for "Visual Studio Code".
fn initials(w: &str, lower: &str, starts: &[usize]) -> bool {
    fn go(w: &str, lower: &str, starts: &[usize]) -> bool {
        if w.is_empty() {
            return true;
        }
        let Some((&first, rest)) = starts.split_first() else { return false };
        let word: &str = &lower[first..rest.first().copied().unwrap_or(lower.len())];
        // Take one or more letters from this word, then go on with the next words.
        let common = word.chars().zip(w.chars()).take_while(|(a, b)| a == b).count();
        (1..=common).rev().any(|k| {
            let cut = w.char_indices().nth(k).map_or(w.len(), |(i, _)| i);
            go(&w[cut..], lower, rest)
        })
    }
    (0..starts.len()).any(|i| go(w, lower, &starts[i..]))
}

/// Byte offsets (in the lowercased name, which has the same offsets for ASCII) where its words
/// start: after a space or punctuation, and at a capital after a small letter ("PowerShell").
fn word_starts(name: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut prev: Option<char> = None;
    let mut offset = 0;
    for c in name.chars() {
        let start = match prev {
            None => c.is_alphanumeric(),
            Some(p) => c.is_alphanumeric() && (!p.is_alphanumeric() || (p.is_lowercase() && c.is_uppercase()) || (p.is_alphabetic() != c.is_alphabetic())),
        };
        if start {
            starts.push(offset);
        }
        // Lowercasing can change a character's length (rarely); keep offsets into the lowercased name.
        offset += c.to_lowercase().map(char::len_utf8).sum::<usize>();
        prev = Some(c);
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(q: &str, name: &str) -> Option<i32> {
        Query::new(q).score(name)
    }

    #[test]
    fn better_matches_score_higher() {
        let exact = s("code", "Code").unwrap();
        let prefix = s("vis", "Visual Studio Code").unwrap();
        let word = s("studio", "Visual Studio Code").unwrap();
        let initials = s("vsc", "Visual Studio Code").unwrap();
        let inside = s("isua", "Visual Studio Code").unwrap();
        let scattered = s("vsdc", "Visual Studio Code").unwrap();
        assert!(exact > prefix && prefix > word && word > initials && initials > inside && inside > scattered, "{exact} {prefix} {word} {initials} {inside} {scattered}");
    }

    #[test]
    fn every_word_has_to_match() {
        assert!(s("studio code", "Visual Studio Code").is_some());
        assert!(s("code studio", "Visual Studio Code").is_some());
        assert!(s("studio word", "Visual Studio Code").is_none());
        assert!(s("", "anything").is_none());
        assert!(s("xyz", "Notepad").is_none());
        // One or two letters only count at a word's start or inside, not scattered.
        assert!(s("ep", "Notepad").is_some());
        assert!(s("np", "Notepad").is_none());
    }

    #[test]
    fn shorter_names_win_ties() {
        assert!(s("note", "Notepad").unwrap() > s("note", "Notepad++ Portable Edition").unwrap());
    }

    #[test]
    fn words_and_initials() {
        assert_eq!(word_starts("PowerShell ISE"), [0, 5, 11]);
        assert_eq!(word_starts("my_file-2.txt"), [0, 3, 8, 10]);
        // Above search's floor for short queries (600), as word starts and initials are.
        assert!(s("ps", "PowerShell").unwrap() >= 600);
        assert!(s("vscode", "Visual Studio Code").unwrap() >= 600);
        assert!(s("wt", "Windows Terminal").unwrap() >= 600);
        assert!(s("te", "Windows Terminal").unwrap() >= 600);
        assert!(s("ep", "Notepad").unwrap() < 600);
        assert_eq!(gaps("vsc", "visual studio code"), Some(2));
        assert_eq!(gaps("vis", "visual"), Some(0));
    }

    #[test]
    fn not_just_ascii() {
        assert!(s("café", "Café Menü.pdf").is_some());
        assert!(s("menü", "Café Menü.pdf").unwrap() >= 650);
        assert!(s("ÉCOLE", "école.docx").is_some());
    }
}
