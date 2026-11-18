use crate::protocol::{MAX_EVENTS, Midi, RATE};

pub const START: u64 = RATE as u64 * 24 / 5;
pub const LENGTH: u64 = RATE as u64 * 192 / 5;
pub const TAIL: u64 = RATE as u64 * 10;
const BEAT: u64 = RATE as u64 * 3 / 5;

pub use crate::protocol::panic_events;

pub fn events(position: u64, frames: usize, looping: bool, output: &mut Vec<Midi>) {
    output.clear();
    for offset in 0..frames {
        let elapsed = position + offset as u64;
        if !looping && elapsed >= LENGTH {
            continue;
        }
        let t = elapsed % LENGTH;
        let beat = t / BEAT;
        let phase = t % BEAT;
        let pitch = 60 + ((beat / 4) % 4) as u8 * 2;
        let mut push = |bytes| {
            output.push(Midi {
                offset: offset as u32,
                bytes,
            })
        };
        if phase == 37 {
            for note in [pitch, pitch + 4, pitch + 7] {
                push([0x90, note, 84]);
            }
        }
        if phase == BEAT / 2 + 37 {
            for note in [pitch, pitch + 4, pitch + 7] {
                push([0x80, note, 0]);
            }
        }
        if phase == 0 {
            push([0xb0, 1, ((beat * 7) % 128) as u8]);
            push([0xe0, 0, if beat % 8 < 4 { 64 } else { 66 }]);
            push([0xb0, 64, if beat.is_multiple_of(4) { 100 } else { 0 }]);
        }
        if t == LENGTH - 1 {
            // Release notes and sustain while preserving effect tails in the render.
            for event in panic_events()
                .into_iter()
                .filter(|event| event.bytes[1] != 120)
            {
                push(event.bytes);
            }
        }
    }
    assert!(output.len() <= MAX_EVENTS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::BLOCK;
    #[test]
    fn range_end_releases_sustain_without_killing_effect_tails() {
        let mut output = Vec::new();
        events(LENGTH - 1, 1, false, &mut output);
        assert_eq!(output.len(), 32);
        assert!(output.iter().all(|event| event.bytes[1] != 120));
        assert_eq!(
            output.iter().filter(|event| event.bytes[1] == 64).count(),
            16
        );
    }

    #[test]
    fn events_cross_block_boundaries_without_duplicates_or_offset_loss() {
        let mut found = Vec::new();
        for start in (0..BEAT as usize).step_by(BLOCK) {
            let mut block = Vec::new();
            events(
                start as u64,
                BLOCK.min(BEAT as usize - start),
                false,
                &mut block,
            );
            for event in block {
                found.push((start as u64 + event.offset as u64, event.bytes));
            }
        }
        let ons: Vec<_> = found.iter().filter(|(_, b)| b[0] == 0x90).collect();
        assert_eq!(ons.len(), 3);
        assert!(ons.iter().all(|(sample, _)| *sample == 37));
        assert_eq!(found.iter().filter(|(_, b)| b[0] == 0x80).count(), 3);
        let mut tail = Vec::new();
        events(LENGTH, BLOCK, false, &mut tail);
        assert!(tail.is_empty());
    }
}
