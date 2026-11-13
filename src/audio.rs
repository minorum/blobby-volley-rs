use std::io::Cursor;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

pub struct SoundEngine {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    pub enabled: bool,
}

impl SoundEngine {
    pub fn new(enabled: bool) -> Self {
        match OutputStream::try_default() {
            Ok((stream, handle)) => Self {
                _stream: Some(stream),
                handle: Some(handle),
                enabled,
            },
            Err(_) => Self {
                _stream: None,
                handle: None,
                enabled,
            },
        }
    }

    pub fn play_wav(&self, wav_bytes: &[u8]) {
        if !self.enabled {
            return;
        }
        let Some(handle) = &self.handle else {
            return;
        };
        let owned = wav_bytes.to_vec();
        if let Ok(source) = Decoder::new(Cursor::new(owned)) {
            if let Ok(sink) = Sink::try_new(handle) {
                sink.append(source);
                sink.detach();
            }
        }
    }
}
