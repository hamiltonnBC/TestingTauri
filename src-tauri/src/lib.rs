use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use tauri::{Manager, State};
use serde::{Serialize, Deserialize};
use axum::{
    routing::get,
    Router,
    Json,
    extract::State as AxumState,
    response::Html,
};
use tower_http::cors::CorsLayer;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimelineEvent {
    id: i64,
    event_date: String,
    title: String,
    description: String,
    image_url: String,
}

#[derive(Deserialize)]
pub struct AddEventRequest {
    event_date: String,
    title: String,
    description: String,
    image_url: String,
}

struct AppState {
    db: Arc<Mutex<Connection>>,
}

#[tauri::command]
fn get_events(state: State<AppState>) -> Result<Vec<TimelineEvent>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = db.prepare("SELECT id, event_date, title, description, image_url FROM events ORDER BY event_date ASC")
        .map_err(|e| e.to_string())?;
        
    let event_iter = stmt.query_map([], |row| {
        Ok(TimelineEvent {
            id: row.get(0)?,
            event_date: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            image_url: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    
    let mut events = Vec::new();
    for event in event_iter {
        events.push(event.map_err(|e| e.to_string())?);
    }
    
    Ok(events)
}

// AXUM HANDLERS
async fn api_get_events(
    AxumState(db): AxumState<Arc<Mutex<Connection>>>
) -> Result<Json<Vec<TimelineEvent>>, String> {
    let db = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare("SELECT id, event_date, title, description, image_url FROM events ORDER BY event_date ASC")
        .map_err(|e| e.to_string())?;
    let event_iter = stmt.query_map([], |row| {
        Ok(TimelineEvent {
            id: row.get(0)?,
            event_date: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            image_url: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    
    let mut events = Vec::new();
    for event in event_iter {
        events.push(event.map_err(|e| e.to_string())?);
    }
    Ok(Json(events))
}

async fn api_add_event(
    AxumState(db): AxumState<Arc<Mutex<Connection>>>,
    Json(payload): Json<AddEventRequest>,
) -> Result<Json<i64>, String> {
    let db = db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO events (event_date, title, description, image_url) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![payload.event_date, payload.title, payload.description, payload.image_url],
    ).map_err(|e| e.to_string())?;
    
    Ok(Json(db.last_insert_rowid()))
}

async fn serve_uploader() -> Html<&'static str> {
    Html(include_str!("uploader.html"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Determine the safe local data directory for this app depending on OS
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
            let db_path = app_data_dir.join("timeline.db");
            
            let conn = Connection::open(db_path).expect("Failed to open local database");
            
            conn.execute(
                "CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY,
                    event_date TEXT NOT NULL,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL,
                    image_url TEXT NOT NULL
                )",
                [],
            ).expect("Failed to create events table");

            let shared_db = Arc::new(Mutex::new(conn));
            
            // Manage state for Tauri UI boundaries
            app.manage(AppState {
                db: shared_db.clone(),
            });

            // Spawn the Axum server
            let axum_db = shared_db.clone();
            tauri::async_runtime::spawn(async move {
                let axum_app = Router::new()
                    .route("/", get(serve_uploader))
                    .route("/api/events", get(api_get_events).post(api_add_event))
                    .with_state(axum_db)
                    .layer(CorsLayer::permissive());

                let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
                println!("🚀 Local Kiosk Server listening on http://0.0.0.0:8080");
                axum::serve(listener, axum_app).await.unwrap();
            });
            
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_events])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
