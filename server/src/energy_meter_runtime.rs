use std::{
    collections::HashMap,
    io,
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
    time::Duration,
};

use chrono::Utc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task::JoinHandle,
    time::{Instant, sleep},
};

use crate::{
    db::{Database, EnergyMeter, EnergyMeterLatestReadingUpsert, EnergyMeterRuntimeStatusUpdate},
    energy_meter_catalog::{EnergyMeterCatalog, EnergyMeterProfile, EnergyMeterRegister},
    greptime::EnergyMeterMeasurementRecord,
    site_config::SiteEnergyMeter,
};

const SUPERVISOR_TICK: Duration = Duration::from_secs(2);
const ERROR_RETRY_DELAY: Duration = Duration::from_secs(5);
static NEXT_TRANSACTION_ID: AtomicU16 = AtomicU16::new(1);

pub async fn run_energy_meter_runtime(db: Database, catalog: EnergyMeterCatalog) {
    let catalog = Arc::new(catalog);
    let mut workers: HashMap<String, MeterWorker> = HashMap::new();

    loop {
        match db.list_energy_meters().await {
            Ok(meters) => {
                reconcile_workers(&db, Arc::clone(&catalog), &mut workers, meters).await;
            }
            Err(err) => {
                eprintln!("energy meter supervisor list fallito: {err}");
            }
        }

        sleep(SUPERVISOR_TICK).await;
    }
}

async fn reconcile_workers(
    db: &Database,
    catalog: Arc<EnergyMeterCatalog>,
    workers: &mut HashMap<String, MeterWorker>,
    meters: Vec<EnergyMeter>,
) {
    let mut seen_ids = Vec::with_capacity(meters.len());

    for meter in meters {
        let meter_config = site_energy_meter_from_db(&meter);
        seen_ids.push(meter_config.id.clone());

        let needs_restart = workers
            .get(&meter_config.id)
            .map(|worker| worker.config != meter_config)
            .unwrap_or(true);

        if !needs_restart {
            continue;
        }

        if let Some(old_worker) = workers.remove(&meter_config.id) {
            old_worker.handle.abort();
        }

        let handle = tokio::spawn(run_meter_worker(
            db.clone(),
            Arc::clone(&catalog),
            meter_config.clone(),
        ));
        workers.insert(
            meter_config.id.clone(),
            MeterWorker {
                config: meter_config,
                handle,
            },
        );
    }

    workers.retain(|meter_id, worker| {
        let keep = seen_ids.iter().any(|candidate| candidate == meter_id);
        if !keep {
            worker.handle.abort();
        }
        keep
    });
}

async fn run_meter_worker(
    db: Database,
    catalog: Arc<EnergyMeterCatalog>,
    meter: SiteEnergyMeter,
) {
    loop {
        let started_at = Instant::now();
        let attempt_at = Utc::now();

        match poll_meter_once(&catalog, &meter).await {
            Ok(records) => {
                let duration_ms = duration_ms_i64(started_at.elapsed());
                let status = EnergyMeterRuntimeStatusUpdate {
                    meter_id: meter.id.clone(),
                    is_online: true,
                    last_attempt_at: attempt_at,
                    last_ok_at: Some(Utc::now()),
                    last_error: None,
                    consecutive_failures: 0,
                    last_poll_duration_ms: Some(duration_ms),
                };

                if let Err(err) = db.upsert_energy_meter_runtime_status(&status).await {
                    eprintln!("energy meter status upsert fallito {}: {}", meter.id, err);
                }

                if let Err(err) = db
                    .replace_energy_meter_latest_readings(
                        &meter.id,
                        records
                            .iter()
                            .map(|record| EnergyMeterLatestReadingUpsert {
                                meter_id: record.meter_id.clone(),
                                metric_key: record.metric_key.clone(),
                                unit: Some(record.unit.clone()),
                                value_text: record.value_text.clone(),
                                value_num: Some(record.value_num),
                                measured_at: record.measured_at,
                            })
                            .collect(),
                    )
                    .await
                {
                    eprintln!("energy meter latest readings upsert fallito {}: {}", meter.id, err);
                }

                db.record_energy_meter_measurements(&records).await;

                sleep(meter_poll_interval(&meter)).await;
            }
            Err(err) => {
                let duration_ms = duration_ms_i64(started_at.elapsed());
                let status = EnergyMeterRuntimeStatusUpdate {
                    meter_id: meter.id.clone(),
                    is_online: false,
                    last_attempt_at: attempt_at,
                    last_ok_at: None,
                    last_error: Some(err.to_string()),
                    consecutive_failures: 1,
                    last_poll_duration_ms: Some(duration_ms),
                };

                if let Err(db_err) = db.upsert_energy_meter_runtime_status(&status).await {
                    eprintln!(
                        "energy meter status errore upsert fallito {}: {}",
                        meter.id, db_err
                    );
                }

                sleep(ERROR_RETRY_DELAY).await;
            }
        }
    }
}

async fn poll_meter_once(
    catalog: &EnergyMeterCatalog,
    meter: &SiteEnergyMeter,
) -> Result<Vec<EnergyMeterMeasurementRecord>, io::Error> {
    let profile = profile_for_meter(catalog, meter)?;
    let host = meter.host.as_deref().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "host misuratore mancante")
    })?;
    let port = meter.port.unwrap_or(profile.default_port);
    let unit_id = meter.unit_id.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "unit_id misuratore mancante")
    })?;
    let addr = format!("{host}:{port}");
    let mut stream = tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(&addr))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timeout connessione Modbus"))?
        .map_err(|err| io::Error::new(err.kind(), format!("connessione Modbus fallita: {err}")))?;

    let register_blocks = build_register_blocks(&profile.registers)?;
    let mut cached_words: HashMap<(String, u16, u16), Vec<u16>> = HashMap::new();
    let measured_at = Utc::now();
    let mut records = Vec::with_capacity(profile.registers.len());

    for block in &register_blocks {
        let words = read_register_block(
            &mut stream,
            unit_id,
            block.function_code,
            block.start_address,
            block.quantity,
        )
        .await?;
        cached_words.insert(
            (
                block.function.clone(),
                block.start_address,
                block.quantity,
            ),
            words,
        );
    }

    for register in &profile.registers {
        let block = find_block_for_register(register, &register_blocks)?;
        let words = cached_words
            .get(&(block.function.clone(), block.start_address, block.quantity))
            .ok_or_else(|| io::Error::other("blocco registri non trovato"))?;

        let offset = usize::from(register.address - block.start_address);
        let register_words = words
            .get(offset..offset + usize::from(register.length))
            .ok_or_else(|| io::Error::other("slice registri fuori range"))?;
        let value_num = decode_register_value(register_words, &register.data_type, &register.endianness)?
            * register.scale;

        records.push(EnergyMeterMeasurementRecord {
            meter_id: meter.id.clone(),
            metric_key: register.metric_key.clone(),
            unit: register.unit.clone(),
            measured_at,
            value_num,
            value_text: format_measurement_value(value_num),
        });
    }

    Ok(records)
}

fn profile_for_meter<'a>(
    catalog: &'a EnergyMeterCatalog,
    meter: &SiteEnergyMeter,
) -> Result<&'a EnergyMeterProfile, io::Error> {
    let catalog_key = meter.catalog_key.as_deref().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "catalog_key misuratore mancante")
    })?;
    let profile = catalog
        .profiles
        .iter()
        .find(|profile| profile.key == catalog_key)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("profilo catalogo sconosciuto: {catalog_key}"),
            )
        })?;

    if profile.transport != "modbus_tcp" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("transport non supportato: {}", profile.transport),
        ));
    }

    Ok(profile)
}

#[derive(Debug, Clone)]
struct RegisterBlock {
    function: String,
    function_code: u8,
    start_address: u16,
    quantity: u16,
}

fn build_register_blocks(registers: &[EnergyMeterRegister]) -> Result<Vec<RegisterBlock>, io::Error> {
    let mut by_function: HashMap<String, Vec<&EnergyMeterRegister>> = HashMap::new();
    for register in registers {
        by_function
            .entry(register.function.clone())
            .or_default()
            .push(register);
    }

    let mut blocks = Vec::new();
    for (function, mut items) in by_function {
        items.sort_by_key(|register| register.address);
        let function_code = function_code(&function)?;
        let mut current: Option<RegisterBlock> = None;

        for register in items {
            let end_address = register
                .address
                .checked_add(register.length)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "overflow range registri")
                })?;

            match current.as_mut() {
                Some(block)
                    if block.start_address + block.quantity == register.address
                        && end_address - block.start_address <= 125 =>
                {
                    block.quantity = end_address - block.start_address;
                }
                Some(block) => {
                    blocks.push(block.clone());
                    current = Some(RegisterBlock {
                        function: function.clone(),
                        function_code,
                        start_address: register.address,
                        quantity: register.length,
                    });
                }
                None => {
                    current = Some(RegisterBlock {
                        function: function.clone(),
                        function_code,
                        start_address: register.address,
                        quantity: register.length,
                    });
                }
            }
        }

        if let Some(block) = current.take() {
            blocks.push(block);
        }
    }

    Ok(blocks)
}

fn find_block_for_register<'a>(
    register: &EnergyMeterRegister,
    blocks: &'a [RegisterBlock],
) -> Result<&'a RegisterBlock, io::Error> {
    blocks
        .iter()
        .find(|block| {
            block.function == register.function
                && register.address >= block.start_address
                && register.address + register.length <= block.start_address + block.quantity
        })
        .ok_or_else(|| io::Error::other("blocco per registro non trovato"))
}

async fn read_register_block(
    stream: &mut TcpStream,
    unit_id: u8,
    function_code: u8,
    start_address: u16,
    quantity: u16,
) -> Result<Vec<u16>, io::Error> {
    let transaction_id = NEXT_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed);
    let request = [
        (transaction_id >> 8) as u8,
        transaction_id as u8,
        0,
        0,
        0,
        6,
        unit_id,
        function_code,
        (start_address >> 8) as u8,
        start_address as u8,
        (quantity >> 8) as u8,
        quantity as u8,
    ];

    tokio::time::timeout(Duration::from_secs(3), stream.write_all(&request))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timeout write Modbus"))??;

    let mut header = [0_u8; 7];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut header))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timeout header Modbus"))??;

    let response_transaction_id = u16::from_be_bytes([header[0], header[1]]);
    if response_transaction_id != transaction_id {
        return Err(io::Error::other("transaction id Modbus inatteso"));
    }

    let length = u16::from_be_bytes([header[4], header[5]]);
    if length < 3 {
        return Err(io::Error::other("payload Modbus troppo corto"));
    }

    let mut payload = vec![0_u8; usize::from(length - 1)];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut payload))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timeout payload Modbus"))??;

    let response_function = payload[0];
    if response_function == function_code | 0x80 {
        let exception_code = payload.get(1).copied().unwrap_or_default();
        return Err(io::Error::other(format!(
            "eccezione Modbus fn=0x{function_code:02x} code=0x{exception_code:02x}"
        )));
    }
    if response_function != function_code {
        return Err(io::Error::other("function code Modbus inatteso"));
    }

    let byte_count = usize::from(payload.get(1).copied().unwrap_or_default());
    if byte_count != usize::from(quantity) * 2 {
        return Err(io::Error::other("byte count Modbus inatteso"));
    }

    let data = payload
        .get(2..2 + byte_count)
        .ok_or_else(|| io::Error::other("payload registri incompleto"))?;
    let mut words = Vec::with_capacity(usize::from(quantity));
    for chunk in data.chunks_exact(2) {
        words.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    Ok(words)
}

fn function_code(function: &str) -> Result<u8, io::Error> {
    match function {
        "holding" => Ok(0x03),
        "input" => Ok(0x04),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("function Modbus non supportata: {other}"),
        )),
    }
}

fn decode_register_value(words: &[u16], data_type: &str, endianness: &str) -> Result<f64, io::Error> {
    let bytes = words_to_bytes(words, endianness)?;
    match data_type {
        "u16" => {
            let bytes: [u8; 2] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "u16 attende 2 byte"))?;
            Ok(f64::from(u16::from_be_bytes(bytes)))
        }
        "i16" => {
            let bytes: [u8; 2] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "i16 attende 2 byte"))?;
            Ok(f64::from(i16::from_be_bytes(bytes)))
        }
        "u32" => {
            let bytes: [u8; 4] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "u32 attende 4 byte"))?;
            Ok(u32::from_be_bytes(bytes) as f64)
        }
        "i32" => {
            let bytes: [u8; 4] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "i32 attende 4 byte"))?;
            Ok(f64::from(i32::from_be_bytes(bytes)))
        }
        "f32" => {
            let bytes: [u8; 4] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "f32 attende 4 byte"))?;
            Ok(f32::from_be_bytes(bytes) as f64)
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("data_type non supportato: {other}"),
        )),
    }
}

fn words_to_bytes(words: &[u16], endianness: &str) -> Result<Vec<u8>, io::Error> {
    let mut ordered_words = words.to_vec();
    let parts: Vec<&str> = endianness.split('-').collect();
    if parts.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("endianness non valida: {endianness}"),
        ));
    }

    let word_order = parts[0];
    let byte_order = parts[1];

    if word_order == "little" {
        ordered_words.reverse();
    } else if word_order != "big" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("word order non supportato: {word_order}"),
        ));
    }

    let mut bytes = Vec::with_capacity(ordered_words.len() * 2);
    for word in ordered_words {
        let pair = if byte_order == "little" {
            word.to_le_bytes()
        } else if byte_order == "big" {
            word.to_be_bytes()
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("byte order non supportato: {byte_order}"),
            ));
        };
        bytes.extend_from_slice(&pair);
    }

    Ok(bytes)
}

fn meter_poll_interval(meter: &SiteEnergyMeter) -> Duration {
    Duration::from_millis(meter.poll_interval_ms.unwrap_or(1_000))
}

fn format_measurement_value(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.3}")
    }
}

fn duration_ms_i64(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn site_energy_meter_from_db(meter: &EnergyMeter) -> SiteEnergyMeter {
    SiteEnergyMeter {
        id: meter.id.clone(),
        name: meter.name.clone(),
        catalog_key: meter.catalog_key.clone(),
        host: meter.host.clone(),
        port: meter.port.and_then(|value| u16::try_from(value).ok()),
        unit_id: meter.unit_id.and_then(|value| u8::try_from(value).ok()),
        poll_interval_ms: meter.poll_interval_ms.and_then(|value| u64::try_from(value).ok()),
        meter_label: meter.meter_label.clone(),
        max_current_a: meter.max_current_a,
        notes: meter.notes.clone(),
    }
}

struct MeterWorker {
    config: SiteEnergyMeter,
    handle: JoinHandle<()>,
}
