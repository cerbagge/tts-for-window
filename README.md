# TTS for Window (v2.0.0)

Edge TTS(무료, API 키 불필요)로 음성을 생성해서 가상 마이크(VB-Cable)로 내보내는 Windows 프로그램입니다.
Discord, 게임, OBS 등에서 마이크처럼 사용할 수 있습니다.

## 다운로드
- 최신 버전: [Releases 페이지](https://github.com/cerbagge/tts-for-window/releases/latest)
- v2.0.0 바로 받기: [tts-for-window-v2.0.0-windows-x64.exe](https://github.com/cerbagge/tts-for-window/releases/download/v2.0.0/tts-for-window-v2.0.0-windows-x64.exe) (Windows 10/11 x64, 설치 필요 없음)

## 준비
1. VB-Cable 설치: https://vb-audio.com/Cable/ (설치 후 재부팅)

## 사용법
1. 내려받은 exe 실행 (직접 빌드했다면 `target\release\tts-for-window.exe`)
2. 음성 선택 ("불러오기" → "목록"), 설정은 `%APPDATA%\tts-for-window\config.json`에 자동 저장
3. 출력 (가상 마이크) → `CABLE Input (VB-Audio Virtual Cable)` 선택
4. 모니터 (내 스피커) → 내 헤드셋/스피커 선택 (내가 듣는 용도)
5. Discord 설정 → 음성 및 비디오 → 입력 장치 → `CABLE Output (VB-Audio Virtual Cable)`
6. 텍스트 입력 후 Enter

## 빌드
```
cargo build --release
cargo test -- --include-ignored   # Edge 실제 합성 테스트 포함
```

## 아이콘
`assets/icon.ico`(exe 내장), `assets/icon.png`(창 아이콘). 다시 만들려면 `python assets/make_icon.py assets`

## 참고
Edge TTS 연동 방식(Sec-MS-GEC v2 토큰 등)은 Python edge-tts 프로젝트를 Rust로 옮긴 것입니다.
출처: https://github.com/rany2/edge-tts
Microsoft가 비공식 엔드포인트의 규칙을 바꾸면 동작이 멈출 수 있습니다.


## 라이선스
- 프로젝트 전체: MIT ([LICENSE](LICENSE))
- 예외: `src/edge_drm.rs`는 edge-tts의 `drm.py`를 옮긴 코드이므로 LGPL-3.0 ([LICENSES/LGPL-3.0.txt](LICENSES/LGPL-3.0.txt), [LICENSES/GPL-3.0.txt](LICENSES/GPL-3.0.txt))
