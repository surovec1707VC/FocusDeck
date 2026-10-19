//! Energy-based voice activity detection with hangover, so short pauses
//! inside a phrase don't end the utterance. Good enough for push-to-talk and
//! quiet rooms; a model-based VAD (e.g. Silero) can replace it later.
// Used once the capture backend lands.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    SpeechStart,
    SpeechEnd,
}

#[derive(Debug, Clone)]
pub struct EnergyVad {
    /// RMS level (0..1 of i16 full scale) above which a frame counts as speech.
    threshold: f32,
    /// Consecutive speech frames needed to start an utterance.
    start_frames: u32,
    /// Consecutive silent frames needed to end it.
    end_frames: u32,
    speaking: bool,
    run: u32,
}

impl EnergyVad {
    pub fn new(threshold: f32, start_frames: u32, end_frames: u32) -> Self {
        Self { threshold, start_frames, end_frames, speaking: false, run: 0 }
    }

    pub fn rms(frame: &[i16]) -> f32 {
        if frame.is_empty() {
            return 0.0;
        }
        let sum: f64 = frame.iter().map(|&s| (s as f64 / i16::MAX as f64).powi(2)).sum();
        (sum / frame.len() as f64).sqrt() as f32
    }

    /// Feeds one frame; returns an event when speech starts or ends.
    pub fn push(&mut self, frame: &[i16]) -> Option<VadEvent> {
        let voiced = Self::rms(frame) >= self.threshold;
        if voiced != self.speaking {
            self.run += 1;
        } else {
            self.run = 0;
        }
        let needed = if self.speaking { self.end_frames } else { self.start_frames };
        if self.run >= needed {
            self.speaking = !self.speaking;
            self.run = 0;
            return Some(if self.speaking { VadEvent::SpeechStart } else { VadEvent::SpeechEnd });
        }
        None
    }
}

impl Default for EnergyVad {
    /// 30 ms frames: ~90 ms of speech to start, ~600 ms of silence to stop.
    fn default() -> Self {
        Self::new(0.02, 3, 20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(amplitude: i16) -> Vec<i16> {
        (0..480).map(|i| if i % 2 == 0 { amplitude } else { -amplitude }).collect()
    }

    #[test]
    fn detects_start_and_end_with_hangover() {
        let mut vad = EnergyVad::new(0.02, 2, 3);
        let loud = frame(8000);
        let quiet = frame(50);

        assert_eq!(vad.push(&quiet), None);
        assert_eq!(vad.push(&loud), None);
        assert_eq!(vad.push(&loud), Some(VadEvent::SpeechStart));
        // A short pause does not end the utterance.
        assert_eq!(vad.push(&quiet), None);
        assert_eq!(vad.push(&loud), None);
        assert_eq!(vad.push(&quiet), None);
        assert_eq!(vad.push(&quiet), None);
        assert_eq!(vad.push(&quiet), Some(VadEvent::SpeechEnd));
    }

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(EnergyVad::rms(&[0; 100]), 0.0);
        assert_eq!(EnergyVad::rms(&[]), 0.0);
    }
}
