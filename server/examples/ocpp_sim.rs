use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::{env, error::Error};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    client_async,
    tungstenite::{
        Message,
        http::{Request, header::SEC_WEBSOCKET_PROTOCOL},
    },
};

#[derive(Clone, Copy, Debug)]
enum OcppVersion {
    V16,
    V201,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cfg = parse_args()?;
    let url = format!("ws://{}/ocpp/{}", cfg.host, cfg.station_id);

    let mut request = Request::builder().uri(&url);
    request = request.header(
        SEC_WEBSOCKET_PROTOCOL,
        match cfg.version {
            OcppVersion::V16 => "ocpp1.6",
            OcppVersion::V201 => "ocpp2.0.1",
        },
    );
    let request = request.body(())?;

    let stream = TcpStream::connect(&cfg.host).await?;
    let (mut ws, _response) = client_async(request, stream).await?;

    let unique_id = "boot-1";
    let boot = match cfg.version {
        OcppVersion::V16 => json!([
            2,
            unique_id,
            "BootNotification",
            {
                "chargePointVendor": "DemoVendor",
                "chargePointModel": "DemoModel",
                "chargePointSerialNumber": "CP-001-SN"
            }
        ]),
        OcppVersion::V201 => json!([
            2,
            unique_id,
            "BootNotification",
            {
                "reason": "PowerUp",
                "chargingStation": {
                    "model": "DemoModel",
                    "vendorName": "DemoVendor",
                    "serialNumber": "CP-001-SN"
                }
            }
        ]),
    };

    ws.send(Message::Text(boot.to_string())).await?;

    if let Some(msg) = ws.next().await {
        let msg = msg?;
        println!("{}", msg.into_text()?);
    } else {
        eprintln!("no response");
    }

    Ok(())
}

struct Config {
    host: String,
    station_id: String,
    version: OcppVersion,
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut host = "127.0.0.1:9000".to_string();
    let mut station_id = "CP-001".to_string();
    let mut version = OcppVersion::V16;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().ok_or("missing value for --host")?,
            "--station" => station_id = args.next().ok_or("missing value for --station")?,
            "--version" => {
                version = match args.next().ok_or("missing value for --version")?.as_str() {
                    "1.6" | "v1.6" => OcppVersion::V16,
                    "2.0.1" | "v2.0.1" => OcppVersion::V201,
                    other => return Err(format!("unsupported version: {other}").into()),
                };
            }
            other => return Err(format!("unknown arg: {other}").into()),
        }
    }

    Ok(Config {
        host,
        station_id,
        version,
    })
}
