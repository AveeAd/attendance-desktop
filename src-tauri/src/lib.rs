mod commands;
mod db;
mod error;
mod mobile;
mod models;
mod reporting;

use db::Db;
use mobile::MobileServer;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db = Db::open(app.handle())?;
            app.manage(db);
            app.manage(MobileServer::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_employees,
            commands::get_employee,
            commands::create_employee,
            commands::update_employee,
            commands::set_employee_active,
            commands::clock_event,
            commands::list_events,
            commands::create_manual_event,
            commands::delete_event,
            commands::get_employee_statuses,
            commands::list_shifts,
            commands::create_shift,
            commands::update_shift,
            commands::delete_shift,
            commands::list_assignments,
            commands::create_assignment,
            commands::delete_assignment,
            commands::get_schedule,
            commands::get_timesheet,
            commands::export_csv,
            commands::export_xlsx,
            commands::get_settings,
            commands::update_settings,
            commands::set_admin_pin,
            commands::verify_admin_pin,
            commands::clear_admin_pin,
            commands::backup_database,
            commands::start_mobile_server,
            commands::stop_mobile_server,
            commands::mobile_server_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Attendance");
}
