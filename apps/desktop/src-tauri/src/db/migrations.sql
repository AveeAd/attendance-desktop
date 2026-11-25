PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS employees (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    department  TEXT,
    role        TEXT,
    active      INTEGER NOT NULL DEFAULT 1,
    pin_hash    TEXT    NOT NULL DEFAULT '',
    qr_token    TEXT    NOT NULL UNIQUE,
    created_at  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS time_events (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    employee_id        INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    event_type         TEXT    NOT NULL CHECK (event_type IN ('clock_in', 'clock_out', 'break_start', 'break_end')),
    occurred_at        TEXT    NOT NULL,
    occurred_at_epoch  INTEGER NOT NULL,
    source             TEXT    NOT NULL DEFAULT 'scan',
    note               TEXT,
    created_at         TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_employee_epoch ON time_events(employee_id, occurred_at_epoch);
CREATE INDEX IF NOT EXISTS idx_events_epoch ON time_events(occurred_at_epoch);

CREATE TABLE IF NOT EXISTS shifts (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT    NOT NULL,
    start_time     TEXT    NOT NULL,
    end_time       TEXT    NOT NULL,
    days_mask      INTEGER NOT NULL DEFAULT 127,
    break_minutes  INTEGER NOT NULL DEFAULT 0,
    active         INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS shift_assignments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    employee_id     INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    shift_id        INTEGER NOT NULL REFERENCES shifts(id) ON DELETE CASCADE,
    effective_from  TEXT    NOT NULL,
    effective_to    TEXT
);

CREATE INDEX IF NOT EXISTS idx_assignments_employee ON shift_assignments(employee_id, effective_from);

CREATE TABLE IF NOT EXISTS duty_rosters (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    employee_id  INTEGER NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    duty_date    TEXT    NOT NULL,
    shift_id     INTEGER REFERENCES shifts(id) ON DELETE SET NULL,
    duty         TEXT    NOT NULL,
    notes        TEXT,
    source       TEXT    NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'generated')),
    created_at   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_duty_roster_date ON duty_rosters(duty_date);
CREATE INDEX IF NOT EXISTS idx_duty_roster_employee_date ON duty_rosters(employee_id, duty_date);

CREATE TABLE IF NOT EXISTS settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_ids (
    kind       TEXT    NOT NULL,
    local_id   INTEGER NOT NULL,
    remote_id  TEXT    NOT NULL,
    version    TEXT    NOT NULL DEFAULT '',
    PRIMARY KEY (kind, local_id)
);

CREATE INDEX IF NOT EXISTS idx_cloud_ids_remote ON cloud_ids(kind, remote_id);

CREATE TABLE IF NOT EXISTS outbox (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    idempotency_key TEXT    NOT NULL UNIQUE,
    kind            TEXT    NOT NULL,
    operation       TEXT    NOT NULL,
    local_id        INTEGER NOT NULL,
    payload         TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'pending',
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    created_at      TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_outbox_status ON outbox(status, id);

CREATE TABLE IF NOT EXISTS cloud_conflicts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT    NOT NULL,
    local_id    INTEGER NOT NULL,
    message     TEXT    NOT NULL,
    occurred_at TEXT    NOT NULL
);

INSERT OR IGNORE INTO settings(key, value) VALUES
    ('company_name', 'My Organization'),
    ('overtime_daily_hours', '8'),
    ('overtime_weekly_hours', '40'),
    ('idle_lock_minutes', '15'),
    ('admin_pin_hash', ''),
    ('cloud_api_url', ''),
    ('cloud_api_key', ''),
    ('cloud_device_id', ''),
    ('cloud_cursor', '0'),
    ('cloud_imported', ''),
    ('cloud_last_sync', ''),
    ('cloud_last_error', '');
