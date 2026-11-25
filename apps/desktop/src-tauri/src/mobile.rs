use std::collections::HashMap;
use std::net::{IpAddr, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server};

use crate::db::{duty_roster, employees, events, lock_conn, timestamp_or_now, Db};
use crate::error::{AppError, AppResult};
use crate::models::EventType;
use crate::reporting::{build_statuses, compute_periods};

#[derive(Debug, Clone, Serialize)]
pub struct MobileServerState {
    pub running: bool,
    pub url: Option<String>,
    pub port: Option<u16>,
}

impl Default for MobileServerState {
    fn default() -> Self {
        Self {
            running: false,
            url: None,
            port: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileEmployee {
    id: i64,
    name: String,
    has_pin: bool,
    state: String,
    clock_in_at: Option<String>,
    worked_minutes: i64,
    today_duty: Option<MobileDuty>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileDuty {
    duty: String,
    shift_name: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClockRequest {
    employee_id: i64,
    code: String,
    pin: String,
    event_type: String,
}

pub struct MobileServer {
    state: Mutex<MobileServerState>,
    stop_signal: Mutex<Option<Sender<()>>>,
}

impl Default for MobileServer {
    fn default() -> Self {
        Self {
            state: Mutex::new(MobileServerState::default()),
            stop_signal: Mutex::new(None),
        }
    }
}

impl MobileServer {
    pub fn status(&self) -> AppResult<MobileServerState> {
        self.state
            .lock()
            .map_err(|_| AppError::InvalidInput("Mobile server state is unavailable".to_string()))
            .map(|state| state.clone())
    }

    pub fn start(&self, db: Db) -> AppResult<MobileServerState> {
        if self.status()?.running {
            return Err(AppError::InvalidInput("Mobile clock-in is already running".to_string()));
        }

        let (server, port) = bind_server()?;
        let (stop_sender, stop_receiver) = mpsc::channel::<()>();
        let ip = detect_lan_ip().unwrap_or_else(|| "127.0.0.1".parse().unwrap());
        let url = format!("http://{ip}:{port}/");

        thread::spawn(move || serve(server, db, stop_receiver));

        let mut state = self.state.lock().map_err(|_| {
            AppError::InvalidInput("Mobile server state is unavailable".to_string())
        })?;
        *state = MobileServerState {
            running: true,
            url: Some(url),
            port: Some(port),
        };
        let mut signal = self.stop_signal.lock().map_err(|_| {
            AppError::InvalidInput("Mobile server state is unavailable".to_string())
        })?;
        *signal = Some(stop_sender);
        Ok(state.clone())
    }

    pub fn stop(&self) -> AppResult<MobileServerState> {
        if let Some(sender) = self.stop_signal.lock().unwrap().take() {
            let _ = sender.send(());
        }
        let mut state = self.state.lock().unwrap();
        *state = MobileServerState::default();
        Ok(state.clone())
    }
}

fn bind_server() -> AppResult<(Server, u16)> {
    let mut last_error = None;
    for port in 8787..=8797 {
        match Server::http(("0.0.0.0", port)) {
            Ok(server) => return Ok((server, port)),
            Err(error) => last_error = Some(error),
        }
    }
    Err(AppError::InvalidInput(format!(
        "Could not bind a mobile server port: {:?}",
        last_error
    )))
}

fn detect_lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|address| address.ip())
}

fn serve(server: Server, db: Db, stop: Receiver<()>) {
    loop {
        if stop.try_recv().is_ok() {
            break;
        }
        match server.recv_timeout(Duration::from_millis(400)) {
            Ok(Some(request)) => {
                let response = handle_request(request, &db);
                if let Err(error) = response {
                    eprintln!("Mobile request failed: {error}");
                }
            }
            Ok(None) => continue,
            Err(error) => {
                eprintln!("Mobile server stopped: {error}");
                break;
            }
        }
    }
}

fn handle_request(mut request: tiny_http::Request, db: &Db) -> AppResult<()> {
    let path = request.url().split('?').next().unwrap_or("/").to_string();

    if *request.method() == Method::Get && (path == "/" || path == "/index.html") {
        let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
            .map_err(|_| AppError::InvalidInput("Invalid HTML response header".to_string()))?;
        return Ok(request.respond(Response::from_string(MOBILE_HTML).with_header(header))?);
    }

    if *request.method() == Method::Get && path == "/api/employees" {
        let payload = json_response(200, &employee_list(db)?)?;
        return Ok(request.respond(payload)?);
    }

    if *request.method() == Method::Post && path == "/api/clock" {
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body)?;
        let input: ClockRequest = serde_json::from_str(&body)
            .map_err(|e| AppError::InvalidInput(format!("Invalid clock request: {e}")))?;
        let result = clock_in_out(db, &input);
        match result {
            Ok(value) => {
                let payload = json_response(200, &value)?;
                return Ok(request.respond(payload)?);
            }
            Err(error) => {
                let payload = json_response(400, &serde_json::json!({ "error": error.to_string() }))?;
                return Ok(request.respond(payload)?);
            }
        }
    }

    Ok(request.respond(Response::from_string("Not found").with_status_code(404))?)
}

fn json_response<T: Serialize>(status: u16, value: &T) -> AppResult<Response<std::io::Cursor<Vec<u8>>>> {
    let body = serde_json::to_string(value)
        .map_err(|e| AppError::InvalidInput(format!("Cannot encode response: {e}")))?;
    let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
        .map_err(|_| AppError::InvalidInput("Invalid JSON response header".to_string()))?;
    Ok(Response::from_string(body).with_status_code(status).with_header(header))
}

fn employee_list(db: &Db) -> AppResult<Vec<MobileEmployee>> {
    let conn = lock_conn(db)?;
    let employees = employees::list(&conn, true)?;
    let today = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .unwrap()
        .timestamp();
    let todays_events = events::list(&conn, None, Some(today), None, 5000)?;
    let statuses = build_statuses(&employees, &todays_events);

    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let records = duty_roster::list_for_range(&conn, &today_str, &today_str)?;
    let shifts = crate::db::shifts::list_shifts(&conn, false)?;
    let mut duties: HashMap<i64, MobileDuty> = HashMap::new();
    for record in records {
        let shift = record
            .shift_id
            .and_then(|id| shifts.iter().find(|shift| shift.id == id));
        duties.entry(record.employee_id).or_insert(MobileDuty {
            duty: record.duty.clone(),
            shift_name: shift
                .map(|shift| shift.name.clone())
                .or_else(|| record.shift_name.clone()),
            start_time: shift.map(|shift| shift.start_time.clone()),
            end_time: shift.map(|shift| shift.end_time.clone()),
        });
    }

    Ok(statuses
        .into_iter()
        .map(|status| MobileEmployee {
            id: status.employee.id,
            name: status.employee.name,
            has_pin: status.employee.has_pin,
            state: status.state,
            clock_in_at: status.clock_in_at,
            worked_minutes: status.open_minutes,
            today_duty: duties.get(&status.employee.id).cloned(),
        })
        .collect())
}

fn clock_in_out(db: &Db, input: &ClockRequest) -> AppResult<serde_json::Value> {
    let event_type = match input.event_type.as_str() {
        "clock_in" => EventType::ClockIn,
        "clock_out" => EventType::ClockOut,
        _ => return Err(AppError::InvalidInput("Unsupported action".to_string())),
    };
    let conn = lock_conn(db)?;
    let employee = employees::get(&conn, input.employee_id)?;
    if !employee.active {
        return Err(AppError::InvalidInput("Employee is inactive".to_string()));
    }
    if !employee.code.eq_ignore_ascii_case(input.code.trim()) {
        return Err(AppError::InvalidInput("Incorrect employee code".to_string()));
    }
    if !employees::verify_pin(&employee, &input.pin) {
        return Err(AppError::IncorrectPin);
    }

    let today = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .unwrap()
        .timestamp();
    let todays_events = events::list(&conn, Some(employee.id), Some(today), None, 500)?;
    let open_period = compute_periods(&todays_events)
        .into_iter()
        .any(|period| period.is_open);

    match event_type {
        EventType::ClockIn if open_period => {
            return Err(AppError::InvalidInput("Already clocked in".to_string()));
        }
        EventType::ClockOut if !open_period => {
            return Err(AppError::InvalidInput("Not clocked in yet".to_string()));
        }
        _ => {}
    }

    let (occurred_at, epoch) = timestamp_or_now(None)?;
    events::insert(
        &conn,
        employee.id,
        event_type,
        &occurred_at,
        epoch,
        "mobile",
        None,
    )?;

    Ok(serde_json::json!({
        "message": if event_type == EventType::ClockIn { "Clocked in" } else { "Clocked out" },
        "state": if event_type == EventType::ClockIn { "in" } else { "out" },
    }))
}

const MOBILE_HTML: &str = include_str!("mobile.html");
