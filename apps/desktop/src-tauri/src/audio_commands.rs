use meraki_audio::{AudioEngine, AudioState, Peaks};
use tauri::State;
#[tauri::command]
pub fn get_audio_state(audio: State<'_, AudioEngine>) -> AudioState {
    audio.state()
}
#[tauri::command]
pub fn get_audio_peaks(
    audio: State<'_, AudioEngine>,
) -> std::collections::BTreeMap<meraki_core::AssetId, Peaks> {
    audio.peaks()
}
#[tauri::command]
pub fn get_eq_response(eq: [f32; 3]) -> Result<Vec<[f64; 2]>, String> {
    if eq
        .iter()
        .any(|v| !v.is_finite() || !(-12. ..=12.).contains(v))
    {
        return Err("EQ inválido".into());
    }
    Ok((0..141)
        .map(|i| {
            let hz = 20. * 1000f64.powf(f64::from(i) / 140.);
            [hz, meraki_audio::response_db(eq, hz)]
        })
        .collect())
}
