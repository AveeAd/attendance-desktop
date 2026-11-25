# Attendance

[![Build desktop apps](https://github.com/AveeAd/attendance-desktop/actions/workflows/build.yml/badge.svg)](https://github.com/AveeAd/attendance-desktop/actions/workflows/build.yml)

A local-first desktop app for employee attendance tracking. Built with Tauri v2, React, TypeScript and SQLite. All data stays on the computer running the app.

## Downloads

Download the latest installers from **[GitHub Releases](https://github.com/AveeAd/attendance-desktop/releases/latest)**. Released builds include macOS (Apple Silicon and Intel), Windows, and Linux installers, and can be downloaded on any device without signing in.

Builds run only when a version tag (`v*`, for example `v0.1.0`) is pushed from a commit on `main`; the tag publishes a public release with all platform installers. The macOS builds are unsigned until code signing and notarization are configured; if macOS blocks them, right-click the app and choose **Open**.

## Features

- **Clock station** — scan a badge/QR code or type an employee ID to clock in, clock out, start a break or end a break
- **Phone clock-in** — one shared QR code opens a LAN page where employees clock in/out from their phone
- **Employee profiles** — codes, departments, roles, printable QR badges, optional per-employee PIN, activate/deactivate
- **Break tracking** — break time is subtracted from worked hours automatically
- **Timesheet reports** — daily records, worked hours, overtime, filters, CSV and Excel export
- **Shifts & schedules** — recurring shifts, employee assignments, expected vs actual times, late/early/missed markers
- **Admin PIN** — salted Argon2 hash, gates admin screens with optional idle auto-lock
- **Backup** — one-click SQLite database snapshot
- **Cross-platform** — macOS, Windows and Linux targets via Tauri

## Requirements

- Node.js 20.19+ (Node 24 recommended)
- Rust 1.77+ via [rustup](https://rustup.rs)
- Platform webview build tools (Tauri prerequisites)

## Getting started

```bash
npm install
npm run desktop:tauri dev
```

To work on the React UI without the Rust shell:

```bash
npm run desktop:dev
```

The UI alone cannot call the database commands outside the Tauri shell, so use `npm run desktop:tauri dev` for a working app.

Run the API server locally with:

```bash
DATABASE_URL=postgres://... API_KEY=your-secret npm run server:dev
```

## Checks

```bash
npm run desktop:build                                             # TypeScript + Vite build
npm run desktop:test                                              # frontend tests
npm run server:build                                              # API server build
npm run server:test                                               # API server tests
cargo test --lib --manifest-path apps/desktop/src-tauri/Cargo.toml # Rust tests
npm run desktop:tauri build                                        # platform installers in apps/desktop/src-tauri/target/release/bundle
```

## Phone clock-in (shared QR)

The desktop app can host a small local web page that phones open over the same Wi-Fi. There is **one shared QR code for the whole team** — it is not tied to a particular employee.

1. Open **Dashboard** on the desktop app.
2. Click **Start mobile clock-in**. The app binds the first free port from `8787` to `8797` and shows a QR code plus the LAN URL (for example `http://192.168.1.23:8787/`).
3. Print or display the QR. Employees scan it with their phone camera and open the page.
4. On the phone, the employee picks their name from the list, types their employee code, and — if one is configured — their PIN. They can then **Clock in** or **Clock out** and see their current state and open hours.
5. The page refreshes employee status every 15 seconds; the desktop dashboard also refreshes live status.
6. Click **Stop mobile clock-in** to shut the server down. It also stops when the app exits.

Events created from phones are saved with the source `mobile` and use the desktop computer's local time, so they appear in **Today's activity** and in reports alongside badge scans.

### Network & security notes

- Phones must be on the **same Wi-Fi / LAN** as the computer. No internet connection or cloud service is involved.
- The QR contains only the LAN URL — there are no secrets encoded in it.
- Access is protected by the **employee code**, plus an optional **per-employee PIN** set on the Employees page (4+ digits, stored as an Argon2 hash).
- The server listens on all interfaces on the local network only. It is **not** a public/internet-facing service; do not port-forward it or put it behind a reverse proxy.
- Duplicate actions are rejected: clocking in when already in, or clocking out when not in, returns an error instead of writing a bad event.
- If the computer has no LAN address available, the URL falls back to `127.0.0.1`, which only works on that computer.
- A firewall may prompt to allow the app to accept incoming connections; allow it on private networks only.

## Usage notes

1. Open **Employees** and add staff. The employee code is what a badge scanner types or a printed QR badge encodes. Optionally set a mobile PIN for each employee.
2. Open an employee's **QR badge** action and print the card for desktop badge scans.
3. At the **Dashboard** clock station, scan or type the code and press Enter. Choose the action button first if it is not a clock-in.
4. Start the **Phone clock-in** server on the Dashboard to show the shared QR for mobile clock in/out.
5. Define shifts and assign them on the **Schedule** page to see expected hours and late/early markers.
6. Use **Reports** to review the selected date range and export CSV or Excel.
7. Set an admin PIN in **Settings** if the computer is shared.

Badge readers that work as keyboard wedges (they type the ID and press Enter) are supported out of the box. Camera-based QR scanning happens on the phone via the shared LAN link; the desktop station uses badge scans or typed IDs.

## Where data is stored

Tauri resolves the platform app data directory and stores a single SQLite file there:

- macOS: `~/Library/Application Support/com.attendance.desktop/attendance.db`
- Windows: `%APPDATA%\com.attendance.desktop\attendance.db`
- Linux: `~/.local/share/com.attendance.desktop/attendance.db`

Use **Settings → Back up database** to save a snapshot anywhere you like.

The mobile feature adds a `pin_hash` column to the `employees` table. Existing databases are upgraded automatically on launch.

## Project layout

```
apps/
  desktop/
    src/              React frontend
      components/     shared UI, app shell, PIN gate
      lib/            typed Tauri command wrappers and formatting helpers
      pages/          dashboard, employees, schedule, reports, settings
      store/          zustand stores (auth, toasts)
    src-tauri/        Rust backend
      src/commands.rs all Tauri commands exposed to the frontend
      src/db/         SQLite connection, migrations, repositories
      src/mobile.rs   LAN HTTP server and mobile clock-in API
      src/mobile.html standalone phone clock-in page served by the LAN server
      src/reporting.rs event-to-period logic, timesheet and schedule calculations
      icons/          generated app icons (source: assets/app-icon.png)
  server/
    src/              Fastify API, Postgres schema, auth and sync routes
    render.yaml       Render Web Service + Postgres blueprint
```

## Design decisions

- Time events are stored with an RFC 3339 timestamp plus a Unix epoch value, so sorting and filtering are correct across timezones while the original local offset is retained for display.
- Worked periods are derived from an ordered event stream with a state machine that ignores duplicate scans and orphan breaks.
- Overtime thresholds are configurable and weekly overtime is allocated to the last row of each employee/ISO week.
- Admin PINs and optional employee PINs are hashed with Argon2 and never stored in plain text.
- The phone page is plain HTML/CSS/JS embedded in the Rust binary — no extra frontend build or hosted assets are needed for mobile access.
- The SQLite connection is wrapped in `Arc<Mutex<Connection>>` so the Tauri commands and LAN server thread share one local database safely.
