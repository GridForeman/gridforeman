use dotenvy::dotenv;
mod api;
mod app_state;
mod badges;
mod db;
mod energy_meter_catalog;
mod greptime;
mod ocpp_runtime;
mod ocpp_v16;
mod ocpp_v201;
mod realtime;
mod site_config;
mod users;

use app_state::ConnectionRegistry;
use db::Database;
use energy_meter_catalog::load_catalog;
use ocpp_runtime::handle_connection;
use realtime::RealtimeNotifier;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    let db = Database::connect_from_env().await?;
    let energy_meter_catalog = load_catalog()?;
    let connections = ConnectionRegistry::default();
    let notifier = RealtimeNotifier::default();
    let api_db = db.clone();
    let api_energy_meter_catalog = energy_meter_catalog.clone();
    let api_connections = connections.clone();
    let api_notifier = notifier.clone();
    tokio::spawn(async move {
        if let Err(err) = api::run_api_server(
            api_db,
            api_energy_meter_catalog,
            api_connections,
            api_notifier,
        )
        .await
        {
            eprintln!("API server chiuso con errore: {err}");
        }
    });

    let bind_addr = "0.0.0.0:9000";
    let listener = TcpListener::bind(bind_addr).await?;
    println!("OCPP server ascolta su ws://{bind_addr}/ocpp/<station_id>");

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(handle_connection(
            stream,
            peer,
            db.clone(),
            connections.clone(),
            notifier.clone(),
        ));
    }
}
