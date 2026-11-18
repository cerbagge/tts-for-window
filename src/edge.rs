//! Microsoft Edge "Read aloud" TTS (no API key). Same protocol as the
//! Python `edge-tts` project.
use serde_json::Value;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::Message;

const TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const CHROMIUM_FULL: &str = "143.0.3650.75";
const CHROMIUM_MAJOR: &str = "143";
const BASE: &str = "speech.platform.bing.com/consumer/speech/synthesize/readaloud";

fn id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn user_agent() -> String {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/{CHROMIUM_MAJOR}.0.0.0 Safari/537.36 Edg/{CHROMIUM_MAJOR}.0.0.0"
    )
}

fn timestamp() -> String {
    chrono::Utc::now()
        .format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)")
        .to_string()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// "ko-KR-SunHiNeural" -> "Microsoft Server Speech Text to Speech Voice (ko-KR, SunHiNeural)"
fn full_voice_name(short: &str) -> String {
    let parts: Vec<&str> = short.splitn(3, '-').collect();
    if parts.len() == 3 {
        format!(
            "Microsoft Server Speech Text to Speech Voice ({}-{}, {})",
            parts[0], parts[1], parts[2]
        )
    } else {
        short.to_string()
    }
}

/// Returns MP3 bytes. `rate` is a multiplier (1.0 = normal), `pitch` is in semitones.
pub fn synthesize(voice: &str, text: &str, rate: f32, pitch: f32) -> Result<Vec<u8>, String> {
    let url = format!(
        "wss://{BASE}/edge/v1?TrustedClientToken={TOKEN}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version=2-{CHROMIUM_FULL}",
        id(),
        crate::edge_drm::sec_ms_gec()
    );
    let mut req = url.into_client_request().map_err(|e| e.to_string())?;
    let h = req.headers_mut();
    let mut set = |k: &'static str, v: String| {
        if let Ok(v) = HeaderValue::from_str(&v) {
            h.insert(k, v);
        }
    };
    set("Pragma", "no-cache".into());
    set("Cache-Control", "no-cache".into());
    set("Origin", "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold".into());
    set("User-Agent", user_agent());
    set("Accept-Language", "en-US,en;q=0.9".into());
    set("Cookie", format!("muid={};", id().to_uppercase()));

    let (mut ws, _) = tungstenite::connect(req).map_err(|e| format!("Edge 연결 실패: {e}"))?;

    let config = format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
         {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\
         \"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}\r\n",
        timestamp()
    );
    ws.send(Message::Text(config)).map_err(|e| e.to_string())?;

    // Edge accepts pitch in Hz only; ~9 Hz per semitone is close enough for speech.
    let ssml = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{}'><prosody pitch='{:+.0}Hz' rate='{:+.0}%' volume='+0%'>{}</prosody></voice></speak>",
        xml_escape(&full_voice_name(voice)),
        pitch * 9.0,
        (rate - 1.0) * 100.0,
        xml_escape(text)
    );
    let msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{}Z\r\nPath:ssml\r\n\r\n{ssml}",
        id(),
        timestamp()
    );
    ws.send(Message::Text(msg)).map_err(|e| e.to_string())?;

    let mut audio = Vec::new();
    loop {
        match ws.read().map_err(|e| format!("Edge 수신 오류: {e}"))? {
            Message::Text(t) if t.contains("Path:turn.end") => break,
            Message::Binary(b) if b.len() >= 2 => {
                let hlen = u16::from_be_bytes([b[0], b[1]]) as usize;
                if b.len() < 2 + hlen {
                    continue;
                }
                let header = String::from_utf8_lossy(&b[2..2 + hlen]);
                if header.contains("Path:audio") {
                    audio.extend_from_slice(&b[2 + hlen..]);
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    let _ = ws.close(None);
    if audio.is_empty() {
        return Err("Edge가 음성을 반환하지 않았습니다. 음성 이름을 확인해 주십시오.".into());
    }
    Ok(audio)
}

/// Returns short voice names, filtered by locale when `lang` is not empty.
pub fn list_voices(lang: &str) -> Result<Vec<String>, String> {
    let url = format!(
        "https://{BASE}/voices/list?trustedclienttoken={TOKEN}&Sec-MS-GEC={}&Sec-MS-GEC-Version=2-{CHROMIUM_FULL}",
        crate::edge_drm::sec_ms_gec()
    );
    let resp = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", user_agent())
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    let mut names: Vec<String> = v
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|x| lang.is_empty() || x["Locale"].as_str() == Some(lang))
                .filter_map(|x| x["ShortName"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    /// Network test: `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn synthesize_and_list() {
        let voices = super::list_voices("ko-KR").expect("voice list");
        assert!(voices.iter().any(|v| v == "ko-KR-SunHiNeural"), "{voices:?}");
        let mp3 = super::synthesize("ko-KR-SunHiNeural", "테스트입니다 & <ok>", 1.2, 2.0).expect("synth");
        assert!(mp3.len() > 1000, "{} bytes", mp3.len());
        rodio::Decoder::new(std::io::Cursor::new(mp3)).expect("valid mp3");
    }
}

#[cfg(test)]
mod cut_tests {
    /// Network test: decoded length must match the MP3 size (48 kbps CBR).
    #[test]
    #[ignore]
    fn decodes_full_length() {
        use rodio::Source;
        let text = "안녕하세요. 이 문장은 중간에 소리가 끊기는지 확인하기 위한 조금 긴 테스트 문장입니다. 끝까지 들리면 정상입니다.";
        let mp3 = super::synthesize("ko-KR-SunHiNeural", text, 1.0, 0.0).expect("synth");
        let expected = mp3.len() as f64 / 6000.0; // 48 kbps = 6000 bytes/s
        let dec = rodio::Decoder::new(std::io::Cursor::new(mp3)).expect("mp3");
        let (rate, ch) = (dec.sample_rate() as f64, dec.channels() as f64);
        let decoded = dec.count() as f64 / rate / ch;
        println!("expected {expected:.2}s, decoded {decoded:.2}s");
        assert!(decoded > expected * 0.95, "decoded {decoded:.2}s < expected {expected:.2}s");
    }
}
