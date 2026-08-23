//! Voice selection, filtering and caching.

use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::models::Voice;

const CACHE_TTL: Duration = Duration::from_secs(60);

/// Whether `filter` is a locale prefix of `language`: both `pt` and `pt-BR` match a `pt-BR`
/// voice, while `US` does not match `en-US`.
///
/// A substring match here made `getVoices("US")` return the `en-US` voices and disagree with
/// what `speak({ language: "US" })` would pick.
pub fn language_matches(language: &str, filter: &str) -> bool {
    language.to_lowercase().starts_with(&filter.to_lowercase())
}

/// Applies the optional locale filter using the same rule `speak` uses to resolve a
/// `language` to a voice.
pub fn filter_by_language(voices: &[Voice], filter: Option<&str>) -> Vec<Voice> {
    voices
        .iter()
        .filter(|voice| match filter {
            Some(filter) => language_matches(&voice.language, filter),
            None => true,
        })
        .cloned()
        .collect()
}

/// Picks the voice a request asks for: an explicit ID wins, otherwise the first voice whose
/// locale matches `language`.
pub fn select<'a>(
    voices: &'a [Voice],
    voice_id: Option<&str>,
    language: Option<&str>,
) -> Option<&'a Voice> {
    match voice_id {
        Some(id) => voices.iter().find(|voice| voice.id == id),
        None => {
            let language = language?;
            voices
                .iter()
                .find(|voice| language_matches(&voice.language, language))
        }
    }
}

/// Explains, for a request that resolved to no voice, what the caller should be told.
/// Returns `None` when the caller asked for nothing in particular.
pub fn unavailable_warning(voice_id: Option<&str>, language: Option<&str>) -> Option<String> {
    match (voice_id, language) {
        (Some(id), _) => Some(format!("Voice '{id}' not found, using default voice")),
        (None, Some(language)) => Some(format!(
            "Language '{language}' has no installed voice, using default voice"
        )),
        (None, None) => None,
    }
}

/// Time-limited cache of the engine's voice list, which is expensive to enumerate.
#[derive(Default)]
pub struct VoiceCache(RwLock<Option<(Vec<Voice>, Instant)>>);

impl VoiceCache {
    pub fn get(&self) -> Option<Vec<Voice>> {
        let entry = self.0.read().ok()?;
        let (voices, cached_at) = entry.as_ref()?;
        (cached_at.elapsed() < CACHE_TTL).then(|| voices.clone())
    }

    /// Stores `voices`, ignoring an empty list: speech-dispatcher reports no voices while it
    /// is still starting, and caching that blanks out `getVoices` for a full TTL after the
    /// daemon is ready.
    pub fn store(&self, voices: &[Voice]) {
        if voices.is_empty() {
            return;
        }
        if let Ok(mut entry) = self.0.write() {
            *entry = Some((voices.to_vec(), Instant::now()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voices(languages: &[&str]) -> Vec<Voice> {
        languages
            .iter()
            .map(|language| Voice {
                id: (*language).into(),
                name: (*language).into(),
                language: (*language).into(),
            })
            .collect()
    }

    fn ids(voices: &[Voice]) -> Vec<&str> {
        voices.iter().map(|voice| voice.id.as_str()).collect()
    }

    #[test]
    fn a_filter_matches_bare_and_full_locale_tags() {
        assert!(language_matches("pt-BR", "pt"));
        assert!(language_matches("pt-BR", "pt-BR"));
        assert!(language_matches("pt-BR", "PT-br"));
        assert!(!language_matches("en-US", "pt"));
        assert!(!language_matches("pt-BR", "pt-PT"));
    }

    /// `getVoices` must select exactly what `speak({ language })` would have picked.
    #[test]
    fn filtering_matches_by_prefix_not_substring() {
        let voices = voices(&["en-US", "pt-BR", "pt-PT"]);

        assert_eq!(ids(&filter_by_language(&voices, None)).len(), 3);
        assert_eq!(
            ids(&filter_by_language(&voices, Some("pt"))),
            ["pt-BR", "pt-PT"]
        );
        assert_eq!(ids(&filter_by_language(&voices, Some("pt-BR"))), ["pt-BR"]);
        assert!(filter_by_language(&voices, Some("US")).is_empty());
    }

    #[test]
    fn an_explicit_voice_id_wins_over_language() {
        let voices = voices(&["en-US", "pt-BR"]);

        let selected = select(&voices, Some("pt-BR"), Some("en-US")).unwrap();
        assert_eq!(selected.id, "pt-BR");
    }

    #[test]
    fn language_selects_the_first_matching_voice() {
        let voices = voices(&["en-US", "pt-BR", "pt-PT"]);

        assert_eq!(select(&voices, None, Some("pt")).unwrap().id, "pt-BR");
        assert_eq!(select(&voices, None, None), None);
        assert_eq!(select(&voices, None, Some("ja")), None);
        assert_eq!(select(&voices, Some("missing"), None), None);
    }

    #[test]
    fn the_warning_names_whichever_the_caller_asked_for() {
        assert!(unavailable_warning(Some("v1"), Some("pt-BR"))
            .unwrap()
            .contains("v1"));
        assert!(unavailable_warning(None, Some("pt-BR"))
            .unwrap()
            .contains("pt-BR"));
        assert_eq!(unavailable_warning(None, None), None);
    }

    #[test]
    fn the_cache_returns_what_was_stored() {
        let cache = VoiceCache::default();
        assert_eq!(cache.get(), None);

        let voices = voices(&["en-US"]);
        cache.store(&voices);
        assert_eq!(cache.get(), Some(voices));
    }

    /// An engine that is still starting reports no voices; caching that would blank out
    /// `getVoices` for the whole TTL.
    #[test]
    fn an_empty_list_is_never_cached() {
        let cache = VoiceCache::default();
        cache.store(&[]);
        assert_eq!(cache.get(), None);

        let voices = voices(&["en-US"]);
        cache.store(&voices);
        cache.store(&[]);
        assert_eq!(cache.get(), Some(voices));
    }
}
