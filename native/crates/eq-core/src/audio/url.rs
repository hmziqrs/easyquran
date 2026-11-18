//! Ayah -> file URL and offsets (web `lib/audio/url.ts`).

use super::catalogue::{AudioRecording, VerseTiming};

const SURAH_COUNT: u16 = 114;

/// `AyahRef`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AyahRef {
    pub surah: u16,
    pub ayah: u16,
}

/// `ResolvedAyahAudio`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedAyahAudio {
    /// Absolute `https://` URL of the file to play.
    pub url: String,
    /// Offset in seconds where this ayah starts inside `url`.
    pub start_sec: f64,
    /// Where it ends; `None` plays to the end of the file.
    pub end_sec: Option<f64>,
}

/// `isSurahNumber`.
pub fn is_surah_number(n: u16) -> bool {
    (1..=SURAH_COUNT).contains(&n)
}

/// `isValidAyahRef`: surah in 1..=114, ayah >= 1, bounded by `ayah_count`
/// when given.
pub fn is_valid_ayah_ref(r: AyahRef, ayah_count: Option<u16>) -> bool {
    if !is_surah_number(r.surah) || r.ayah < 1 {
        return false;
    }
    ayah_count.is_none_or(|count| r.ayah <= count)
}

/// `expandTemplate`: fills `{surah}` and `{ayah}` with 3-digit numbers.
pub fn expand_template(template: &str, r: AyahRef) -> String {
    template
        .replace("{surah}", &format!("{:03}", r.surah))
        .replace("{ayah}", &format!("{:03}", r.ayah))
}

/// `resolveAyahAudio`: playable file and offsets for one ayah, `None` when
/// the ref or timing cannot resolve.
pub fn resolve_ayah_audio(
    recording: &AudioRecording,
    r: AyahRef,
    ayah_count: Option<u16>,
) -> Option<ResolvedAyahAudio> {
    if !is_valid_ayah_ref(r, ayah_count) {
        return None;
    }
    let url = expand_template(&recording.url_template, r);
    match &recording.timing {
        VerseTiming::FilePerAyah => Some(ResolvedAyahAudio {
            url,
            start_sec: 0.0,
            end_sec: None,
        }),
        VerseTiming::SurahFile { ayah_start_ms } => {
            let start = *ayah_start_ms.get(&r.ayah)?;
            let next = ayah_start_ms.get(&(r.ayah + 1));
            Some(ResolvedAyahAudio {
                url,
                start_sec: start as f64 / 1000.0,
                end_sec: next.map(|ms| *ms as f64 / 1000.0),
            })
        }
    }
}

/// `isAllowedAudioUrl`: only `https:` audio with a host is ever loaded.
pub fn is_allowed_audio_url(url: &str) -> bool {
    let Some(scheme_end) = url.find("://") else {
        return false;
    };
    if !url[..scheme_end].eq_ignore_ascii_case("https") {
        return false;
    }
    let rest = &url[scheme_end + 3..];
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..host_end];
    let host = authority.rsplit('@').next().unwrap_or_default();
    !host.is_empty() && !host.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::audio::catalogue::{AudioAttribution, AudioLicence};

    fn recording(timing: VerseTiming) -> AudioRecording {
        AudioRecording {
            id: "r".into(),
            reciter_id: "x".into(),
            label: "l".into(),
            url_template: "https://audio.example.org/{surah}/{surah}{ayah}.mp3".into(),
            timing,
            licence: AudioLicence {
                name: "n".into(),
                url: "https://l".into(),
            },
            attribution: AudioAttribution {
                en: String::new(),
                ar: String::new(),
                url: "https://s".into(),
            },
        }
    }

    #[test]
    fn file_per_ayah_pads_three_digits() {
        let r = recording(VerseTiming::FilePerAyah);
        let got = resolve_ayah_audio(&r, AyahRef { surah: 1, ayah: 7 }, Some(7)).unwrap();
        assert_eq!(got.url, "https://audio.example.org/001/001007.mp3");
        assert_eq!(got.start_sec, 0.0);
        assert_eq!(got.end_sec, None);
        assert!(resolve_ayah_audio(&r, AyahRef { surah: 1, ayah: 8 }, Some(7)).is_none());
        assert!(
            resolve_ayah_audio(
                &r,
                AyahRef {
                    surah: 115,
                    ayah: 1
                },
                None
            )
            .is_none()
        );
        assert!(resolve_ayah_audio(&r, AyahRef { surah: 1, ayah: 0 }, None).is_none());
    }

    #[test]
    fn surah_file_uses_offsets() {
        let starts = BTreeMap::from([(1, 0), (2, 6500)]);
        let r = recording(VerseTiming::SurahFile {
            ayah_start_ms: starts,
        });
        let one = resolve_ayah_audio(&r, AyahRef { surah: 1, ayah: 1 }, None).unwrap();
        assert_eq!((one.start_sec, one.end_sec), (0.0, Some(6.5)));
        let two = resolve_ayah_audio(&r, AyahRef { surah: 1, ayah: 2 }, None).unwrap();
        assert_eq!((two.start_sec, two.end_sec), (6.5, None));
        assert!(resolve_ayah_audio(&r, AyahRef { surah: 1, ayah: 3 }, None).is_none());
    }

    #[test]
    fn only_https_is_allowed() {
        assert!(is_allowed_audio_url("https://a.org/x.mp3"));
        assert!(is_allowed_audio_url("HTTPS://a.org"));
        assert!(!is_allowed_audio_url("http://a.org/x.mp3"));
        assert!(!is_allowed_audio_url("file:///x.mp3"));
        assert!(!is_allowed_audio_url("https:///x"));
        assert!(!is_allowed_audio_url("not a url"));
    }
}
