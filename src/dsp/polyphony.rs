use super::patch::Patch;
use super::voice::Voice;

/// Most notes that can sound at once.
pub const MAX_NOTES: usize = 32;

/// Extra voices, so a stolen note can fade out while the new note starts
/// right away in a spare voice.
const SPARE_VOICES: usize = 8;

/// A fixed pool of voices, so you can play chords.
///
/// The voices live in an array that is created once and reused forever:
/// nothing here allocates memory, which is what makes it safe to call from
/// the real-time audio thread.
pub struct Polyphony {
    voices: [Voice; MAX_NOTES + SPARE_VOICES],
    /// Counts note-ons, to know which voice is the oldest.
    counter: u64,
    /// Keys held down, for mono mode.
    held: HeldNotes,
}

impl Default for Polyphony {
    fn default() -> Self {
        Self {
            voices: std::array::from_fn(|_| Voice::default()),
            counter: 0,
            held: HeldNotes::default(),
        }
    }
}

impl Polyphony {
    pub fn note_on(&mut self, note: u8, velocity: f32, patch: &Patch, sr: f32) {
        if patch.mono {
            self.mono_note_on(note, velocity, patch, sr);
            return;
        }
        self.counter += 1;
        let i = self.pick_voice(note, sr);
        self.voices[i].note_on(note, velocity, self.counter);
        // The new note needs its numbers right away, not at the next block.
        self.voices[i].prepare(patch, sr);
    }

    pub fn note_off(&mut self, note: u8, mono: bool) {
        if mono {
            self.mono_note_off(note);
            return;
        }
        for voice in &mut self.voices {
            if voice.is_playing() && voice.note == note {
                voice.note_off();
            }
        }
    }

    /// Mono mode plays everything on voice 0. If another key is still held,
    /// the new note is played *legato*: the voice slides to the new pitch
    /// without restarting its envelopes.
    fn mono_note_on(&mut self, note: u8, velocity: f32, patch: &Patch, sr: f32) {
        let legato = self.held.last().is_some();
        self.held.push(note);

        let voice = &mut self.voices[0];
        if legato && voice.is_playing() {
            voice.glide_to(note);
        } else {
            self.counter += 1;
            voice.note_on(note, velocity, self.counter);
            voice.prepare(patch, sr);
        }
    }

    /// Releasing the sounding key slides back to the previous key if it's
    /// still held, like on classic mono bass synths.
    fn mono_note_off(&mut self, note: u8) {
        self.held.remove(note);
        let voice = &mut self.voices[0];
        if voice.note != note {
            return; // that key wasn't the one sounding
        }
        match self.held.last() {
            Some(previous) => voice.glide_to(previous),
            None => voice.note_off(),
        }
    }

    /// The note mono mode is playing (it always uses voice 0).
    pub fn mono_note(&self) -> u8 {
        self.voices[0].note
    }

    /// Let every note ring out, e.g. when switching between mono and poly.
    pub fn release_all(&mut self) {
        self.held.clear();
        for voice in &mut self.voices {
            voice.note_off();
        }
    }

    pub fn reset(&mut self) {
        self.held.clear();
        for voice in &mut self.voices {
            voice.reset();
        }
    }

    /// Update every sounding voice after the patch changed.
    pub fn prepare(&mut self, patch: &Patch, sr: f32) {
        for voice in &mut self.voices {
            if voice.is_active() {
                voice.prepare(patch, sr);
            }
        }
    }

    /// Fill `out` with the sum of every sounding voice. `bend` scales every
    /// voice's frequency (see `Voice::render`).
    pub fn render(&mut self, patch: &Patch, bend: f32, out: &mut [f32]) {
        out.fill(0.0);
        for voice in &mut self.voices {
            if voice.is_active() {
                voice.render(patch, bend, out);
            }
        }
    }

    /// Choose which voice plays a new note.
    fn pick_voice(&mut self, note: u8, sr: f32) -> usize {
        // The same note is already playing: re-strike it, like a piano.
        if let Some(i) = self
            .voices
            .iter()
            .position(|v| v.is_playing() && v.note == note)
        {
            return i;
        }

        // Already at the note limit: "steal" the oldest note. It fades out
        // over a few milliseconds in the background instead of being cut
        // off, which would click.
        let playing = self.voices.iter().filter(|v| v.is_playing()).count();
        if playing >= MAX_NOTES
            && let Some(oldest) = self.oldest(|v| v.is_playing())
        {
            self.voices[oldest].fade_out(sr);
        }

        // Normally there's a free voice now (a spare one if we just stole).
        if let Some(i) = self.voices.iter().position(|v| !v.is_active()) {
            return i;
        }

        // Every voice is busy, spares included. This only happens when lots
        // of notes arrive within a few milliseconds: cut the oldest one off.
        let oldest = self.oldest(|_| true).unwrap();
        self.voices[oldest].reset();
        oldest
    }

    /// The oldest voice among those that `include` accepts.
    fn oldest(&self, include: impl Fn(&Voice) -> bool) -> Option<usize> {
        self.voices
            .iter()
            .enumerate()
            .filter(|(_, v)| include(v))
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
    }
}

/// The keys currently held down, oldest first. A fixed-size array, so it
/// never allocates; if more than 16 keys are held, the oldest is forgotten.
#[derive(Default)]
struct HeldNotes {
    notes: [u8; 16],
    count: usize,
}

impl HeldNotes {
    fn push(&mut self, note: u8) {
        self.remove(note);
        if self.count == self.notes.len() {
            self.notes.copy_within(1.., 0);
            self.count -= 1;
        }
        self.notes[self.count] = note;
        self.count += 1;
    }

    fn remove(&mut self, note: u8) {
        if let Some(i) = self.notes[..self.count].iter().position(|&n| n == note) {
            self.notes.copy_within(i + 1..self.count, i);
            self.count -= 1;
        }
    }

    /// The most recently pressed key that's still down.
    fn last(&self) -> Option<u8> {
        self.notes[..self.count].last().copied()
    }

    fn clear(&mut self) {
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::PRESETS;

    const SR: f32 = 96_000.0;

    fn count(poly: &Polyphony, f: impl Fn(&Voice) -> bool) -> usize {
        poly.voices.iter().filter(|v| f(v)).count()
    }

    #[test]
    fn stolen_note_fades_out_while_new_note_starts() {
        let patch = PRESETS[0].1;
        let mut poly = Polyphony::default();
        let mut out = [0.0; 64];

        for note in 0..MAX_NOTES as u8 {
            poly.note_on(40 + note, 1.0, &patch, SR);
        }
        poly.render(&patch, 1.0, &mut out);

        // One note too many: the oldest (note 40) starts fading, but the
        // new note plays in a spare voice right away.
        poly.note_on(90, 1.0, &patch, SR);
        assert_eq!(count(&poly, |v| v.is_playing()), MAX_NOTES);
        assert_eq!(count(&poly, |v| v.is_active()), MAX_NOTES + 1);
        assert!(!poly.voices.iter().any(|v| v.is_playing() && v.note == 40));

        // 3 ms later the stolen voice is free again.
        for _ in 0..(0.004 * SR) as usize / out.len() + 1 {
            poly.render(&patch, 1.0, &mut out);
        }
        assert_eq!(count(&poly, |v| v.is_active()), MAX_NOTES);
    }

    #[test]
    fn mono_slides_between_held_notes() {
        let mut patch = PRESETS[0].1;
        patch.mono = true;
        let mut poly = Polyphony::default();

        poly.note_on(40, 1.0, &patch, SR);
        poly.note_on(43, 1.0, &patch, SR); // legato: same voice slides
        assert_eq!(count(&poly, |v| v.is_playing()), 1);
        assert_eq!(poly.voices[0].note, 43);

        poly.note_off(43, true); // back to the key still held
        assert_eq!(poly.voices[0].note, 40);
        assert!(poly.voices[0].is_playing());

        poly.note_off(40, true); // nothing held: the note releases
        let mut out = [0.0; 64];
        for _ in 0..(5.0 * SR) as usize / out.len() {
            poly.render(&patch, 1.0, &mut out);
        }
        assert_eq!(count(&poly, |v| v.is_active()), 0);
    }

    #[test]
    fn a_flood_of_notes_never_fails() {
        let patch = PRESETS[0].1;
        let mut poly = Polyphony::default();
        for note in 0..=127 {
            poly.note_on(note, 1.0, &patch, SR);
        }
        assert_eq!(count(&poly, |v| v.is_playing()), MAX_NOTES);
    }
}
