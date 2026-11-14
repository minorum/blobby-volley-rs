use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::sync::Arc;

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

    pub fn play_wav(&self, wav_bytes: &Arc<[u8]>) {
        if !self.enabled {
            return;
        }
        let Some(handle) = &self.handle else {
            return;
        };
        if let Ok(source) = Decoder::new(Cursor::new(Arc::clone(wav_bytes))) {
            if let Ok(sink) = Sink::try_new(handle) {
                sink.append(source);
                sink.detach();
            }
        }
    }
}
