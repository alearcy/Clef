use crate::protocol::{Frame, RATE, Result};
use clef_plugin_host::files::NewFile;
use std::{fs::File, io::BufWriter, path::Path};

pub struct Wav {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    output: Option<NewFile>,
    pub frames: u64,
}

impl Wav {
    pub fn create(path: &Path, expected_frames: u64) -> Result<Self> {
        if expected_frames
            .checked_mul(6)
            .is_none_or(|bytes| bytes > u32::MAX as u64 - 128)
        {
            return Err("render exceeds the WAV RIFF size limit".into());
        }
        let output = NewFile::create(path)?;
        let writer = hound::WavWriter::new(
            BufWriter::new(output.file.try_clone()?),
            hound::WavSpec {
                channels: 2,
                sample_rate: RATE,
                bits_per_sample: 24,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        Ok(Self {
            writer: Some(writer),
            output: Some(output),
            frames: 0,
        })
    }
    pub fn write(&mut self, frames: &[Frame]) -> Result<()> {
        for frame in frames {
            for &sample in frame {
                if !sample.is_finite() || !(-1.0..1.0).contains(&sample) {
                    return Err(
                        "non-finite or clipped render; no completed file was published".into(),
                    );
                }
                self.writer
                    .as_mut()
                    .ok_or("WAV already finalized")?
                    .write_sample(
                        (sample * 8_388_608.0)
                            .round()
                            .clamp(-8_388_608.0, 8_388_607.0) as i32,
                    )?;
            }
            self.frames += 1;
        }
        Ok(())
    }
    pub fn finish(mut self, expected: u64) -> Result<()> {
        if self.frames != expected {
            return Err("unexpected WAV frame count".into());
        }
        self.writer
            .take()
            .ok_or("WAV already finalized")?
            .finalize()?;
        self.output.take().ok_or("missing render output")?.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipped_render_never_publishes_a_file() {
        let path = std::env::temp_dir().join(format!("clef-clipped-{}.wav", std::process::id()));
        let mut wav = Wav::create(&path, 1).unwrap();
        assert!(wav.write(&[[1.1, 0.0]]).is_err());
        drop(wav);
        assert!(!path.exists());
    }
    #[test]
    fn finalization_write_failure_never_publishes_a_file() {
        let directory = clef_model::project::TemporaryDirectory::new("wav-write-failure").unwrap();
        let path = directory.0.join("output.wav");
        let mut wav = Wav::create(&path, 1).unwrap();
        let temporary = std::fs::read_dir(&directory.0)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let spec = wav.writer.as_ref().unwrap().spec();
        wav.writer = Some(
            hound::WavWriter::new(BufWriter::new(File::open(&temporary).unwrap()), spec).unwrap(),
        );
        wav.write(&[[0.1; 2]]).unwrap();
        assert!(wav.finish(1).is_err());
        assert!(!path.exists());
        assert!(!temporary.exists());
    }

    #[test]
    fn pcm24_render_has_correct_shape_and_never_overwrites() {
        let path = std::env::temp_dir().join(format!("clef-pcm24-{}.wav", std::process::id()));
        let mut wav = Wav::create(&path, 2).unwrap();
        wav.write(&[[0.25, -0.25], [0.0, 0.5]]).unwrap();
        wav.finish(2).unwrap();
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().bits_per_sample, 24);
        assert_eq!(reader.spec().sample_rate, RATE);
        assert_eq!(reader.duration(), 2);
        assert!(Wav::create(&path, 2).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
