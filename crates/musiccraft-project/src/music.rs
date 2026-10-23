//! Note names and musical time helpers.

use musiccraft_params::TICKS_PER_STEP;

const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Name of a MIDI key, with key 60 = `C5` (octave = key / 12).
#[must_use]
pub fn key_name(key: u8) -> String {
    let name = NAMES.get(usize::from(key % 12)).copied().unwrap_or("?");
    format!("{name}{}", key / 12)
}

/// Parses a key given as a MIDI number (`60`) or a name (`C5`, `F#4`, `Bb3`).
#[must_use]
pub fn parse_key(text: &str) -> Option<u8> {
    if let Ok(number) = text.parse::<u8>() {
        return (number <= 127).then_some(number);
    }
    let mut chars = text.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base: i32 = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (accidental, octave) = if let Some(rest) = rest.strip_prefix('#') {
        (1, rest)
    } else if let Some(rest) = rest.strip_prefix('b') {
        (-1, rest)
    } else {
        (0, rest)
    };
    let octave: i32 = octave.parse().ok()?;
    let key = octave * 12 + base + accidental;
    u8::try_from(key).ok().filter(|k| *k <= 127)
}

/// A chord quality: its name and semitone intervals from the root.
pub type Chord = (&'static str, &'static [u8]);

/// Chord qualities by family, as (heading, chords). Names are single words so
/// they can be typed as a command argument. Extended chords keep every tone
/// up to the named one (an 11th keeps the 9th, a 13th keeps the 9th but drops
/// the 11th, which clashes with the major third).
pub const CHORD_GROUPS: [(&str, &[Chord]); 6] = [
    (
        "Triads",
        &[
            ("major", &[0, 4, 7]),
            ("minor", &[0, 3, 7]),
            ("dim", &[0, 3, 6]),
            ("aug", &[0, 4, 8]),
            ("sus2", &[0, 2, 7]),
            ("sus4", &[0, 5, 7]),
            ("power", &[0, 7]),
            ("power8", &[0, 7, 12]),
            ("majb5", &[0, 4, 6]),
        ],
    ),
    (
        "Sixths",
        &[
            ("6", &[0, 4, 7, 9]),
            ("min6", &[0, 3, 7, 9]),
            ("6/9", &[0, 4, 7, 9, 14]),
            ("min6/9", &[0, 3, 7, 9, 14]),
        ],
    ),
    (
        "Sevenths",
        &[
            ("maj7", &[0, 4, 7, 11]),
            ("min7", &[0, 3, 7, 10]),
            ("dom7", &[0, 4, 7, 10]),
            ("dim7", &[0, 3, 6, 9]),
            ("min7b5", &[0, 3, 6, 10]),
            ("minmaj7", &[0, 3, 7, 11]),
            ("aug7", &[0, 4, 8, 10]),
            ("augmaj7", &[0, 4, 8, 11]),
            ("7b5", &[0, 4, 6, 10]),
            ("maj7b5", &[0, 4, 6, 11]),
            ("7sus2", &[0, 2, 7, 10]),
            ("7sus4", &[0, 5, 7, 10]),
        ],
    ),
    (
        "Ninths",
        &[
            ("add9", &[0, 4, 7, 14]),
            ("minadd9", &[0, 3, 7, 14]),
            ("dom9", &[0, 4, 7, 10, 14]),
            ("maj9", &[0, 4, 7, 11, 14]),
            ("min9", &[0, 3, 7, 10, 14]),
            ("minmaj9", &[0, 3, 7, 11, 14]),
            ("9sus4", &[0, 5, 7, 10, 14]),
            ("7b9", &[0, 4, 7, 10, 13]),
            ("7#9", &[0, 4, 7, 10, 15]),
            ("7#5", &[0, 4, 8, 10]),
            ("7#5b9", &[0, 4, 8, 10, 13]),
        ],
    ),
    (
        "Elevenths",
        &[
            ("add11", &[0, 4, 7, 17]),
            ("minadd11", &[0, 3, 7, 17]),
            ("dom11", &[0, 4, 7, 10, 14, 17]),
            ("maj11", &[0, 4, 7, 11, 14, 17]),
            ("min11", &[0, 3, 7, 10, 14, 17]),
            ("7#11", &[0, 4, 7, 10, 18]),
            ("maj7#11", &[0, 4, 7, 11, 18]),
            ("9#11", &[0, 4, 7, 10, 14, 18]),
        ],
    ),
    (
        "Thirteenths",
        &[
            ("dom13", &[0, 4, 7, 10, 14, 21]),
            ("maj13", &[0, 4, 7, 11, 14, 21]),
            ("min13", &[0, 3, 7, 10, 14, 21]),
            ("13sus4", &[0, 5, 7, 10, 14, 21]),
            ("7b13", &[0, 4, 7, 10, 20]),
            ("13b9", &[0, 4, 7, 10, 13, 21]),
            ("13#11", &[0, 4, 7, 10, 14, 18, 21]),
        ],
    ),
];

/// Every chord quality, family by family.
pub fn chords() -> impl Iterator<Item = Chord> {
    CHORD_GROUPS
        .iter()
        .flat_map(|(_, chords)| chords.iter().copied())
}

/// Scales as (name, semitone degrees from the root).
pub const SCALES: [(&str, &[u8]); 7] = [
    ("major", &[0, 2, 4, 5, 7, 9, 11]),
    ("minor", &[0, 2, 3, 5, 7, 8, 10]),
    ("harmonic minor", &[0, 2, 3, 5, 7, 8, 11]),
    ("dorian", &[0, 2, 3, 5, 7, 9, 10]),
    ("mixolydian", &[0, 2, 4, 5, 7, 9, 10]),
    ("major pentatonic", &[0, 2, 4, 7, 9]),
    ("minor pentatonic", &[0, 3, 5, 7, 10]),
];

/// Intervals of a chord quality by name.
#[must_use]
pub fn chord(name: &str) -> Option<&'static [u8]> {
    chords().find(|(n, _)| *n == name).map(|(_, i)| i)
}

/// Whether `key` belongs to the scale `degrees` built on pitch class `root`.
#[must_use]
pub fn in_scale(key: u8, root: u8, degrees: &[u8]) -> bool {
    degrees.contains(&((key + 12 - root % 12) % 12))
}

/// Converts ticks to steps for display.
#[must_use]
pub fn ticks_to_steps(ticks: u32) -> f64 {
    f64::from(ticks) / f64::from(TICKS_PER_STEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_round_trip() {
        for key in 0..=127 {
            assert_eq!(parse_key(&key_name(key)), Some(key));
        }
        assert_eq!(key_name(60), "C5");
        assert_eq!(key_name(69), "A5");
    }

    #[test]
    fn parses_numbers_names_and_flats() {
        assert_eq!(parse_key("60"), Some(60));
        assert_eq!(parse_key("c5"), Some(60));
        assert_eq!(parse_key("Bb4"), Some(58));
        assert_eq!(parse_key("F#4"), Some(54));
        assert_eq!(parse_key("H2"), None);
        assert_eq!(parse_key("128"), None);
        assert_eq!(parse_key("G10"), Some(127));
        assert_eq!(parse_key("G#10"), None);
        assert_eq!(parse_key("Cb0"), None);
    }

    #[test]
    fn chord_names_are_unique_words_with_rising_intervals() {
        let names: Vec<_> = chords().map(|(n, _)| n).collect();
        assert!(names.len() >= 50);
        for (i, name) in names.iter().enumerate() {
            assert!(!name.contains(char::is_whitespace), "{name}");
            assert!(!names[..i].contains(name), "duplicate {name}");
        }
        for (name, intervals) in chords() {
            assert_eq!(intervals.first(), Some(&0), "{name}");
            assert!(intervals.windows(2).all(|w| w[0] < w[1]), "{name}");
        }
    }

    #[test]
    fn chords_and_scales() {
        assert_eq!(chord("min7"), Some(&[0, 3, 7, 10][..]));
        assert_eq!(chord("nope"), None);
        assert_eq!(chord("13#11"), Some(&[0, 4, 7, 10, 14, 18, 21][..]));
        let major = SCALES[0].1;
        assert!(in_scale(60, 0, major) && in_scale(64, 0, major) && !in_scale(61, 0, major));
        assert!(in_scale(66, 2, major), "F# is in D major");
    }
}
