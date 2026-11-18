use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

pub enum Cmd {
    /// main device, monitor device (None inside = default device; outer None = monitor off)
    SetDevices(Option<String>, Option<Option<String>>),
    Play(Arc<Vec<u8>>, f32, f32),
    Stop,
}

pub fn list_output_devices() -> Vec<String> {
    rodio::cpal::default_host()
        .output_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

struct Out {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,
}

fn open(name: &Option<String>) -> Result<Out, String> {
    let host = rodio::cpal::default_host();
    let device = match name {
        None => host.default_output_device(),
        Some(n) => host
            .output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().ok().as_deref() == Some(n.as_str())),
    }
    .ok_or_else(|| format!("출력 장치를 찾을 수 없습니다: {}", name.as_deref().unwrap_or("기본 장치")))?;
    let (stream, handle) = OutputStream::try_from_device(&device).map_err(|e| e.to_string())?;
    let sink = Sink::try_new(&handle).map_err(|e| e.to_string())?;
    Ok(Out { _stream: stream, handle, sink })
}

fn play(out: &Option<Out>, bytes: &Arc<Vec<u8>>, volume: f32) -> Result<(), String> {
    if let Some(o) = out {
        let src = Decoder::new(Cursor::new(bytes.as_ref().clone())).map_err(|e| e.to_string())?;
        o.sink.set_volume(volume);
        o.sink.append(src);
    }
    Ok(())
}

fn stop(out: &mut Option<Out>) {
    if let Some(o) = out {
        o.sink.stop();
        if let Ok(s) = Sink::try_new(&o.handle) {
            o.sink = s;
        }
    }
}

/// Spawns the audio thread. Errors are reported through `on_error`.
pub fn spawn(on_error: impl Fn(String) + Send + 'static) -> Sender<Cmd> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    std::thread::spawn(move || {
        let mut main: Option<Out> = None;
        let mut monitor: Option<Out> = None;
        for cmd in rx {
            match cmd {
                Cmd::SetDevices(m, mon) => {
                    main = open(&m).map_err(|e| on_error(format!("출력 장치: {e}"))).ok();
                    monitor = match mon {
                        None => None,
                        Some(name) => open(&name).map_err(|e| on_error(format!("모니터 장치: {e}"))).ok(),
                    };
                }
                Cmd::Play(bytes, vol, mon_vol) => {
                    if let Err(e) = play(&main, &bytes, vol).and_then(|_| play(&monitor, &bytes, mon_vol)) {
                        on_error(format!("재생 오류: {e}"));
                    }
                }
                Cmd::Stop => {
                    stop(&mut main);
                    stop(&mut monitor);
                }
            }
        }
    });
    tx
}
