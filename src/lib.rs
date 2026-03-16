pub mod capture;
pub mod capture_fallback;
pub mod cleanup;
pub mod config;
pub mod downloader;
pub mod input_state;
pub mod metadata;
pub mod playlist;
pub mod progress;
pub mod queue_processor;
pub mod settings;
pub mod slides_generator;
pub mod slides_viewer;
pub mod subtitle_detector;
pub mod subtitle_extractor;
pub mod theme;
pub mod url_validator;

use input_state::PipelineState;
use settings::{load_settings, Settings, SettingsState};

pub fn run() {
    // Load settings from config.json (or create defaults)
    let initial_settings = load_settings().unwrap_or_else(|e| {
        eprintln!("Warning: failed to load settings ({e}), using defaults");
        Settings::default()
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SettingsState(std::sync::Mutex::new(initial_settings)))
        .manage(PipelineState::new())
        .invoke_handler(tauri::generate_handler![
            // Settings
            settings::get_settings,
            settings::update_settings,
            settings::validate_settings,
            settings::reset_settings,
            settings::get_config_path,
            // Input state & queue management
            input_state::set_input_state,
            input_state::get_input_state,
            input_state::add_queue_item,
            input_state::get_queue,
            input_state::update_queue_item,
            input_state::remove_queue_item,
            input_state::retry_queue_item,
            input_state::set_language,
            // Slides viewer
            slides_viewer::load_slides_html,
            slides_viewer::list_library_entries,
            slides_viewer::get_slides_metadata,
            slides_viewer::get_slides_path,
            slides_viewer::get_capture_frames,
            slides_viewer::open_slides_external,
            slides_viewer::delete_library_entry,
            slides_viewer::open_folder,
            slides_viewer::recapture_library_item,
            slides_viewer::check_recapture_available,
            slides_viewer::check_video_exists,
            // URL validation
            url_validator::validate_youtube_url,
            // Subtitle detection
            subtitle_detector::check_subtitle_availability,
            // Subtitle extraction (Korean priority → English fallback)
            subtitle_extractor::extract_subtitles_cmd,
            subtitle_extractor::select_subtitle_language,
            // Frame capture
            capture::capture_frames,
            capture::get_scene_threshold,
            // Capture mode fallback
            capture_fallback::resolve_capture_mode_cmd,
            // Queue processing
            queue_processor::start_queue_processing,
            queue_processor::get_processing_status,
            queue_processor::get_item_progress,
            // Playlist detection & fetching
            playlist::detect_playlist_url,
            playlist::fetch_playlist,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
