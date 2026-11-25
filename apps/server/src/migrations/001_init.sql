CREATE TABLE IF NOT EXISTS organizations (
    id          uuid PRIMARY KEY,
    name        text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS api_keys (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    key_hash        text NOT NULL,
    key_hint        text NOT NULL,
    active          boolean NOT NULL DEFAULT true,
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_used_at    timestamptz
);

CREATE INDEX IF NOT EXISTS idx_api_keys_hint ON api_keys(key_hint);

CREATE TABLE IF NOT EXISTS devices (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            text NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_seen_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS employees (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    code            text NOT NULL,
    name            text NOT NULL,
    department      text,
    role            text,
    active          boolean NOT NULL DEFAULT true,
    pin_hash        text NOT NULL DEFAULT '',
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_employees_org_code_lower
    ON employees(organization_id, lower(code));

CREATE TABLE IF NOT EXISTS shifts (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name            text NOT NULL,
    start_time      text NOT NULL,
    end_time        text NOT NULL,
    days            integer[] NOT NULL DEFAULT '{}',
    break_minutes   integer NOT NULL DEFAULT 0,
    active          boolean NOT NULL DEFAULT true,
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS shift_assignments (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    employee_id     uuid NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    shift_id        uuid NOT NULL REFERENCES shifts(id) ON DELETE CASCADE,
    effective_from  date NOT NULL,
    effective_to    date,
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS duty_rosters (
    id              uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    employee_id     uuid NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    duty_date       date NOT NULL,
    shift_id        uuid REFERENCES shifts(id) ON DELETE SET NULL,
    duty            text NOT NULL,
    notes           text,
    source          text NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'generated')),
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_duty_rosters_org_date
    ON duty_rosters(organization_id, duty_date);

CREATE TABLE IF NOT EXISTS time_events (
    id                 uuid PRIMARY KEY,
    organization_id    uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    employee_id        uuid NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    event_type         text NOT NULL CHECK (event_type IN ('clock_in', 'clock_out', 'break_start', 'break_end')),
    occurred_at        timestamptz NOT NULL,
    occurred_at_epoch  bigint NOT NULL,
    source             text NOT NULL DEFAULT 'scan',
    note               text,
    device_id          uuid,
    created_at         timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_events_org_id
    ON time_events(organization_id, id);

CREATE INDEX IF NOT EXISTS idx_events_org_employee_epoch
    ON time_events(organization_id, employee_id, occurred_at_epoch);

CREATE TABLE IF NOT EXISTS settings (
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    key             text NOT NULL,
    value           text NOT NULL,
    updated_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, key)
);

CREATE TABLE IF NOT EXISTS change_log (
    id              bigserial PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    entity          text NOT NULL,
    entity_id       uuid NOT NULL,
    operation       text NOT NULL CHECK (operation IN ('upsert', 'delete')),
    payload         jsonb NOT NULL,
    occurred_at     timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_change_log_org_id
    ON change_log(organization_id, id);
