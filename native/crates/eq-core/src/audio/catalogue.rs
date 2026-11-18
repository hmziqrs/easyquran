//! Recitation source catalogue (web `lib/audio/catalogue.ts`). EMPTY ON
//! PURPOSE, as on the web: no recitation source is licensed yet (O22).
//! While [`baked`] lists no recording, [`AudioCatalogue::enabled`] is false
//! and no audio surface renders.
//!
//! Adding a source mirrors the web's steps: the reciter and recording go in
//! [`baked`] (`url_template` must be `https://`, placeholders `{surah}` and
//! `{ayah}`, 3-digit), its host goes in `native/nsh.json`'s
//! `capabilities.network.hosts`, and a URL test goes next to the ones in
//! `url.rs`. Nothing else in `eq_core::audio` names a reciter or a host.

use std::collections::BTreeMap;

/// `AudioLicence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioLicence {
    /// Short name shown to readers.
    pub name: String,
    /// `https://` link to the licence text.
    pub url: String,
}

/// `AudioAttribution`: credit lines shown verbatim in the full player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioAttribution {
    pub en: String,
    pub ar: String,
    /// `https://` link to the source.
    pub url: String,
}

/// `AudioReciter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioReciter {
    pub id: String,
    pub name: String,
}

/// `VerseTiming`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerseTiming {
    /// Each ayah is its own file.
    FilePerAyah,
    /// One file per surah; ayah number -> start offset in ms.
    SurahFile { ayah_start_ms: BTreeMap<u16, u64> },
}

/// `AudioRecording`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioRecording {
    pub id: String,
    pub reciter_id: String,
    /// Display label, e.g. "Murattal, 128 kbps".
    pub label: String,
    /// Template with `{surah}` and `{ayah}` placeholders.
    pub url_template: String,
    pub timing: VerseTiming,
    pub licence: AudioLicence,
    pub attribution: AudioAttribution,
}

/// `AudioCatalogue`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioCatalogue {
    pub reciters: Vec<AudioReciter>,
    pub recordings: Vec<AudioRecording>,
}

/// The shipped catalogue (`AUDIO_CATALOGUE`): empty until a source is
/// licensed.
pub fn baked() -> AudioCatalogue {
    AudioCatalogue::default()
}

impl AudioCatalogue {
    /// `AUDIO_ENABLED`: gate for every audio surface.
    pub fn enabled(&self) -> bool {
        !self.recordings.is_empty()
    }

    /// `defaultRecordingId`: the first recording listed.
    pub fn default_recording_id(&self) -> Option<&str> {
        self.recordings.first().map(|r| r.id.as_str())
    }

    /// `recordingById`.
    pub fn recording_by_id(&self, id: &str) -> Option<&AudioRecording> {
        self.recordings.iter().find(|r| r.id == id)
    }

    /// `reciterById`.
    pub fn reciter_by_id(&self, id: &str) -> Option<&AudioReciter> {
        self.reciters.iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_catalogue_is_dark() {
        let c = baked();
        assert!(!c.enabled());
        assert_eq!(c.default_recording_id(), None);
    }
}
