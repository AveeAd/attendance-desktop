import { randomUUID } from "node:crypto";
import Fastify, { type FastifyInstance, type FastifyReply, type FastifyRequest } from "fastify";
import type { Pool } from "pg";
import { z } from "zod";

import { apiKeyHint, verifyApiKey } from "./auth.js";
import { validateClockEvent, type ClockEventLike } from "./clock.js";
import { rows } from "./db.js";

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

const optionalText = (value: string | null | undefined): string | null => {
  const trimmed = value?.trim() ?? "";
  return trimmed ? trimmed : null;
};

const optionalUuid = (value: string | null | undefined): string | null => {
  const trimmed = value?.trim() ?? "";
  return trimmed && UUID_RE.test(trimmed) ? trimmed : null;
};

const EmployeeBody = z.object({
  id: z.string().uuid().optional(),
  code: z.string().min(1),
  name: z.string().min(1),
  department: z.string().nullable().optional(),
  role: z.string().nullable().optional(),
  active: z.boolean().optional().default(true),
  pin_hash: z.string().optional().default(""),
  created_at: z.string().optional(),
  updated_at: z.string().optional(),
});

const ShiftBody = z.object({
  id: z.string().uuid().optional(),
  name: z.string().min(1),
  start_time: z.string().min(1),
  end_time: z.string().min(1),
  days: z.array(z.number().int().min(1).max(7)).default([]),
  break_minutes: z.number().int().min(0).optional().default(0),
  active: z.boolean().optional().default(true),
  updated_at: z.string().optional(),
});

const AssignmentBody = z.object({
  id: z.string().uuid().optional(),
  employee_id: z.string().uuid(),
  shift_id: z.string().uuid(),
  effective_from: z.string().min(1),
  effective_to: z.string().nullable().optional(),
  updated_at: z.string().optional(),
});

const DutyRosterBody = z.object({
  id: z.string().uuid().optional(),
  employee_id: z.string().uuid(),
  duty_date: z.string().min(1),
  shift_id: z.string().uuid().nullable().optional(),
  duty: z.string().min(1),
  notes: z.string().nullable().optional(),
  source: z.enum(["manual", "generated"]).optional().default("manual"),
  created_at: z.string().optional(),
  updated_at: z.string().optional(),
});

const EventBody = z.object({
  id: z.string().uuid(),
  employee_id: z.string().uuid(),
  event_type: z.enum(["clock_in", "clock_out", "break_start", "break_end"]),
  occurred_at: z.string().min(1),
  occurred_at_epoch: z.number().int(),
  source: z.string().min(1).optional().default("scan"),
  note: z.string().nullable().optional(),
});

const ImportBody = z.object({
  employees: z.array(EmployeeBody.extend({ id: z.string().uuid() })).default([]),
  shifts: z.array(ShiftBody.extend({ id: z.string().uuid() })).default([]),
  assignments: z.array(AssignmentBody.extend({ id: z.string().uuid() })).default([]),
  duty_rosters: z.array(DutyRosterBody.extend({ id: z.string().uuid() })).default([]),
  events: z.array(EventBody).default([]),
  settings: z.record(z.string(), z.string()).default({}),
});

function orgId(request: FastifyRequest): string {
  return (request as FastifyRequest & { organizationId: string }).organizationId;
}

function deviceId(request: FastifyRequest): string | null {
  const value = request.headers["x-device-id"];
  const first = Array.isArray(value) ? value[0] : value;
  return first && UUID_RE.test(first) ? first : null;
}

function sameVersion(current: Date | string | null, base: string | undefined): boolean {
  if (!base) return true;
  if (!current) return false;
  return Math.abs(new Date(current).getTime() - Date.parse(base)) < 1500;
}

function conflict(reply: FastifyReply, current: unknown) {
  return reply.code(409).send({ error: "conflict", current });
}

export interface AppOptions {
  pool: Pool;
  logger?: boolean;
}

export async function buildServer({ pool, logger = false }: AppOptions): Promise<FastifyInstance> {
  const app = Fastify({ logger });

  app.get("/health", async () => ({ ok: true }));

  app.addHook("onRequest", async (request, reply) => {
    const path = request.url.split("?")[0];
    if (!path.startsWith("/api/v1/")) return;

    const authorization = request.headers.authorization ?? "";
    const match = /^Bearer\s+(.+)$/i.exec(authorization);
    if (!match) return reply.code(401).send({ error: "Missing API key" });
    const key = match[1].trim();

    const candidates = await rows<{ id: string; organization_id: string; key_hash: string }>(
      pool,
      "SELECT id, organization_id, key_hash FROM api_keys WHERE active = true AND key_hint = $1",
      [apiKeyHint(key)],
    );
    const authorized = candidates.find((candidate) => verifyApiKey(key, candidate.key_hash));
    if (!authorized) return reply.code(401).send({ error: "Invalid API key" });

    const device = deviceId(request);
    if (!device) return reply.code(400).send({ error: "A valid X-Device-Id header is required" });

    await pool.query(
      `INSERT INTO devices(id, organization_id, name)
       VALUES ($1, $2, $3)
       ON CONFLICT (id) DO UPDATE SET last_seen_at = now()`,
      [device, authorized.organization_id, "Desktop"],
    );
    await pool.query("UPDATE api_keys SET last_used_at = now() WHERE id = $1", [authorized.id]);

    (request as FastifyRequest & { organizationId: string }).organizationId =
      authorized.organization_id;
  });

  const logChange = async (
    organizationId: string,
    entity: string,
    entityId: string,
    operation: "upsert" | "delete",
    payload: unknown,
  ) => {
    await pool.query(
      `INSERT INTO change_log(organization_id, entity, entity_id, operation, payload)
       VALUES ($1, $2, $3, $4, $5::jsonb)`,
      [organizationId, entity, entityId, operation, JSON.stringify(payload)],
    );
  };

  const currentById = async (table: string, organizationId: string, id: string) => {
    const result = await pool.query(
      `SELECT * FROM ${table} WHERE organization_id = $1 AND id = $2`,
      [organizationId, id],
    );
    return result.rows[0] as Record<string, unknown> | undefined;
  };

  const upsertEmployee = async (
    organizationId: string,
    input: z.infer<typeof EmployeeBody> & { id?: string },
    allowConflictCheck: boolean,
    reply: FastifyReply,
  ) => {
    if (allowConflictCheck && input.id) {
      const current = await currentById("employees", organizationId, input.id);
      if (current && !sameVersion(current.updated_at as Date | string, input.updated_at)) {
        conflict(reply, current);
        return null;
      }
    }
    const id = input.id ?? randomUUID();
    const now = new Date().toISOString();
    try {
      const result = await pool.query(
        `INSERT INTO employees(id, organization_id, code, name, department, role, active, pin_hash, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9::timestamptz, now()), $10::timestamptz)
         ON CONFLICT (id) DO UPDATE SET
           code = excluded.code, name = excluded.name, department = excluded.department,
           role = excluded.role, active = excluded.active, pin_hash = excluded.pin_hash,
           updated_at = excluded.updated_at
         RETURNING *`,
        [
          id,
          organizationId,
          input.code.trim(),
          input.name.trim(),
          optionalText(input.department),
          optionalText(input.role),
          input.active,
          input.pin_hash ?? "",
          input.created_at ?? null,
          input.updated_at && allowConflictCheck ? now : input.updated_at ?? now,
        ],
      );
      const employee = result.rows[0];
      await logChange(organizationId, "employee", id, "upsert", employee);
      return employee;
    } catch (error) {
      if ((error as { code?: string }).code === "23505") {
        conflict(reply, { error: "Employee code is already in use" });
        return null;
      }
      throw error;
    }
  };

  const upsertShift = async (
    organizationId: string,
    input: z.infer<typeof ShiftBody> & { id?: string },
    allowConflictCheck: boolean,
    reply: FastifyReply,
  ) => {
    if (allowConflictCheck && input.id) {
      const current = await currentById("shifts", organizationId, input.id);
      if (current && !sameVersion(current.updated_at as Date | string, input.updated_at)) {
        conflict(reply, current);
        return null;
      }
    }
    const id = input.id ?? randomUUID();
    const now = new Date().toISOString();
    const result = await pool.query(
      `INSERT INTO shifts(id, organization_id, name, start_time, end_time, days, break_minutes, active, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6::integer[], $7, $8, $9::timestamptz)
       ON CONFLICT (id) DO UPDATE SET
         name = excluded.name, start_time = excluded.start_time, end_time = excluded.end_time,
         days = excluded.days, break_minutes = excluded.break_minutes, active = excluded.active,
         updated_at = excluded.updated_at
       RETURNING *`,
      [
        id,
        organizationId,
        input.name.trim(),
        input.start_time,
        input.end_time,
        input.days,
        input.break_minutes,
        input.active,
        allowConflictCheck ? now : input.updated_at ?? now,
      ],
    );
    const shift = result.rows[0];
    await logChange(organizationId, "shift", id, "upsert", shift);
    return shift;
  };

  const upsertAssignment = async (
    organizationId: string,
    input: z.infer<typeof AssignmentBody> & { id?: string },
    allowConflictCheck: boolean,
    reply: FastifyReply,
  ) => {
    if (allowConflictCheck && input.id) {
      const current = await currentById("shift_assignments", organizationId, input.id);
      if (current && !sameVersion(current.updated_at as Date | string, input.updated_at)) {
        conflict(reply, current);
        return null;
      }
    }
    const id = input.id ?? randomUUID();
    const now = new Date().toISOString();
    const result = await pool.query(
      `INSERT INTO shift_assignments(id, organization_id, employee_id, shift_id, effective_from, effective_to, updated_at)
       VALUES ($1, $2, $3, $4, $5::date, $6::date, $7::timestamptz)
       ON CONFLICT (id) DO UPDATE SET
         employee_id = excluded.employee_id, shift_id = excluded.shift_id,
         effective_from = excluded.effective_from, effective_to = excluded.effective_to,
         updated_at = excluded.updated_at
       RETURNING *`,
      [
        id,
        organizationId,
        input.employee_id,
        input.shift_id,
        input.effective_from,
        optionalText(input.effective_to),
        allowConflictCheck ? now : input.updated_at ?? now,
      ],
    );
    const assignment = result.rows[0];
    await logChange(organizationId, "assignment", id, "upsert", assignment);
    return assignment;
  };

  const upsertDutyRoster = async (
    organizationId: string,
    input: z.infer<typeof DutyRosterBody> & { id?: string },
    allowConflictCheck: boolean,
    reply: FastifyReply,
  ) => {
    if (allowConflictCheck && input.id) {
      const current = await currentById("duty_rosters", organizationId, input.id);
      if (current && !sameVersion(current.updated_at as Date | string, input.updated_at)) {
        conflict(reply, current);
        return null;
      }
    }
    const id = input.id ?? randomUUID();
    const now = new Date().toISOString();
    const result = await pool.query(
      `INSERT INTO duty_rosters(id, organization_id, employee_id, duty_date, shift_id, duty, notes, source, created_at, updated_at)
       VALUES ($1, $2, $3, $4::date, $5, $6, $7, $8, COALESCE($9::timestamptz, now()), $10::timestamptz)
       ON CONFLICT (id) DO UPDATE SET
         employee_id = excluded.employee_id, duty_date = excluded.duty_date,
         shift_id = excluded.shift_id, duty = excluded.duty, notes = excluded.notes,
         source = excluded.source, updated_at = excluded.updated_at
       RETURNING *`,
      [
        id,
        organizationId,
        input.employee_id,
        input.duty_date,
        optionalUuid(input.shift_id),
        input.duty.trim(),
        optionalText(input.notes),
        input.source ?? "manual",
        input.created_at ?? null,
        allowConflictCheck ? now : input.updated_at ?? now,
      ],
    );
    const roster = result.rows[0];
    await logChange(organizationId, "duty_roster", id, "upsert", roster);
    return roster;
  };

  app.get("/api/v1/ping", async (request) => ({
    ok: true,
    organization_id: orgId(request),
    device_id: deviceId(request),
  }));

  app.get("/api/v1/bootstrap", async (request) => {
    const organizationId = orgId(request);
    const [organization] = await rows<{ id: string; name: string }>(
      pool,
      "SELECT id, name FROM organizations WHERE id = $1",
      [organizationId],
    );
    const [employees, events, shifts, assignments, dutyRosters, settings, cursorRows] =
      await Promise.all([
        rows(pool, "SELECT * FROM employees WHERE organization_id = $1 ORDER BY name COLLATE \"C\"", [organizationId]),
        rows(pool, "SELECT * FROM time_events WHERE organization_id = $1 ORDER BY occurred_at_epoch", [organizationId]),
        rows(pool, "SELECT * FROM shifts WHERE organization_id = $1 ORDER BY start_time, name", [organizationId]),
        rows(pool, "SELECT * FROM shift_assignments WHERE organization_id = $1 ORDER BY effective_from DESC", [organizationId]),
        rows(pool, "SELECT * FROM duty_rosters WHERE organization_id = $1 ORDER BY duty_date", [organizationId]),
        rows<{ key: string; value: string }>(pool, "SELECT key, value FROM settings WHERE organization_id = $1", [organizationId]),
        rows<{ cursor: string }>(
          pool,
          "SELECT COALESCE(MAX(id), 0)::text AS cursor FROM change_log WHERE organization_id = $1",
          [organizationId],
        ),
      ]);

    return {
      organization,
      cursor: cursorRows[0]?.cursor ?? "0",
      employees,
      events,
      shifts,
      assignments,
      duty_rosters: dutyRosters,
      settings: Object.fromEntries(settings.map((row) => [row.key, row.value])),
    };
  });

  app.get("/api/v1/changes", async (request) => {
    const organizationId = orgId(request);
    const query = z.object({ since: z.string().optional().default("0") }).parse(request.query);
    const since = Number.parseInt(query.since, 10) || 0;
    const changes = await rows(
      pool,
      `SELECT id::text AS id, entity, entity_id::text AS entity_id, operation, payload
       FROM change_log WHERE organization_id = $1 AND id > $2 ORDER BY id LIMIT 2000`,
      [organizationId, since],
    );
    const cursor = changes.length ? changes[changes.length - 1].id : String(since);
    return { cursor, changes };
  });

  app.post("/api/v1/employees", async (request, reply) => {
    const input = EmployeeBody.parse(request.body);
    const employee = await upsertEmployee(orgId(request), input, false, reply);
    return employee ? reply.code(201).send(employee) : reply;
  });

  app.put("/api/v1/employees/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const input = EmployeeBody.parse(request.body);
    const employee = await upsertEmployee(orgId(request), { ...input, id: params.id }, true, reply);
    return employee ? reply.send(employee) : reply;
  });

  app.delete("/api/v1/employees/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const organizationId = orgId(request);
    const result = await pool.query(
      "DELETE FROM employees WHERE organization_id = $1 AND id = $2",
      [organizationId, params.id],
    );
    if (!result.rowCount) return reply.code(404).send({ error: "not_found" });
    await logChange(organizationId, "employee", params.id, "delete", { id: params.id });
    return reply.code(204).send();
  });

  app.post("/api/v1/shifts", async (request, reply) => {
    const input = ShiftBody.parse(request.body);
    const shift = await upsertShift(orgId(request), input, false, reply);
    return shift ? reply.code(201).send(shift) : reply;
  });

  app.put("/api/v1/shifts/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const input = ShiftBody.parse(request.body);
    const shift = await upsertShift(orgId(request), { ...input, id: params.id }, true, reply);
    return shift ? reply.send(shift) : reply;
  });

  app.delete("/api/v1/shifts/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const organizationId = orgId(request);
    const result = await pool.query(
      "DELETE FROM shifts WHERE organization_id = $1 AND id = $2",
      [organizationId, params.id],
    );
    if (!result.rowCount) return reply.code(404).send({ error: "not_found" });
    await logChange(organizationId, "shift", params.id, "delete", { id: params.id });
    return reply.code(204).send();
  });

  app.post("/api/v1/assignments", async (request, reply) => {
    const input = AssignmentBody.parse(request.body);
    const assignment = await upsertAssignment(orgId(request), input, false, reply);
    return assignment ? reply.code(201).send(assignment) : reply;
  });

  app.put("/api/v1/assignments/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const input = AssignmentBody.parse(request.body);
    const assignment = await upsertAssignment(orgId(request), { ...input, id: params.id }, true, reply);
    return assignment ? reply.send(assignment) : reply;
  });

  app.delete("/api/v1/assignments/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const organizationId = orgId(request);
    const result = await pool.query(
      "DELETE FROM shift_assignments WHERE organization_id = $1 AND id = $2",
      [organizationId, params.id],
    );
    if (!result.rowCount) return reply.code(404).send({ error: "not_found" });
    await logChange(organizationId, "assignment", params.id, "delete", { id: params.id });
    return reply.code(204).send();
  });

  app.post("/api/v1/duty-rosters", async (request, reply) => {
    const input = DutyRosterBody.parse(request.body);
    const roster = await upsertDutyRoster(orgId(request), input, false, reply);
    return roster ? reply.code(201).send(roster) : reply;
  });

  app.put("/api/v1/duty-rosters/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const input = DutyRosterBody.parse(request.body);
    const roster = await upsertDutyRoster(orgId(request), { ...input, id: params.id }, true, reply);
    return roster ? reply.send(roster) : reply;
  });

  app.delete("/api/v1/duty-rosters/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const organizationId = orgId(request);
    const result = await pool.query(
      "DELETE FROM duty_rosters WHERE organization_id = $1 AND id = $2",
      [organizationId, params.id],
    );
    if (!result.rowCount) return reply.code(404).send({ error: "not_found" });
    await logChange(organizationId, "duty_roster", params.id, "delete", { id: params.id });
    return reply.code(204).send();
  });

  app.put("/api/v1/settings/:key", async (request, reply) => {
    const params = z.object({ key: z.string().min(1) }).parse(request.params);
    const input = z.object({ value: z.string() }).parse(request.body);
    const organizationId = orgId(request);
    const current = await rows<{ updated_at: Date }>(
      pool,
      "SELECT updated_at FROM settings WHERE organization_id = $1 AND key = $2",
      [organizationId, params.key],
    );
    const base = (request.headers["x-base-version"] as string | undefined) ?? undefined;
    if (current[0] && base && !sameVersion(current[0].updated_at, base)) {
      return conflict(reply, current[0]);
    }
    const updatedAt = base && current[0] ? new Date().toISOString() : current[0]?.updated_at ?? new Date().toISOString();
    await pool.query(
      `INSERT INTO settings(organization_id, key, value, updated_at)
       VALUES ($1, $2, $3, $4::timestamptz)
       ON CONFLICT (organization_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at`,
      [organizationId, params.key, input.value, updatedAt],
    );
    await logChange(organizationId, "setting", params.key, "upsert", {
      key: params.key,
      value: input.value,
    });
    return reply.send({ key: params.key, value: input.value });
  });

  app.post("/api/v1/events", async (request, reply) => {
    const input = EventBody.parse(request.body);
    const organizationId = orgId(request);
    const existing = await rows(
      pool,
      "SELECT * FROM time_events WHERE organization_id = $1 AND id = $2",
      [organizationId, input.id],
    );
    if (existing[0]) return reply.send({ event: existing[0], duplicate: true });

    const employee = await rows(
      pool,
      "SELECT id FROM employees WHERE organization_id = $1 AND id = $2",
      [organizationId, input.employee_id],
    );
    if (!employee[0]) return reply.code(404).send({ error: "employee_not_found" });

    const history = await rows<ClockEventLike>(
      pool,
      "SELECT event_type, occurred_at_epoch::text AS occurred_at_epoch FROM time_events WHERE organization_id = $1 AND employee_id = $2",
      [organizationId, input.employee_id],
    );
    const error = validateClockEvent(input.event_type, history);
    if (error) return reply.code(409).send({ error });

    const result = await pool.query(
      `INSERT INTO time_events(id, organization_id, employee_id, event_type, occurred_at, occurred_at_epoch, source, note, device_id)
       VALUES ($1, $2, $3, $4, $5::timestamptz, $6, $7, $8, $9)
       RETURNING *`,
      [
        input.id,
        organizationId,
        input.employee_id,
        input.event_type,
        input.occurred_at,
        input.occurred_at_epoch,
        input.source,
        optionalText(input.note),
        deviceId(request),
      ],
    );
    const event = result.rows[0];
    await logChange(organizationId, "event", input.id, "upsert", event);
    return reply.code(201).send({ event, duplicate: false });
  });

  app.delete("/api/v1/events/:id", async (request, reply) => {
    const params = z.object({ id: z.string().uuid() }).parse(request.params);
    const organizationId = orgId(request);
    const result = await pool.query(
      "DELETE FROM time_events WHERE organization_id = $1 AND id = $2",
      [organizationId, params.id],
    );
    if (!result.rowCount) return reply.code(404).send({ error: "not_found" });
    await logChange(organizationId, "event", params.id, "delete", { id: params.id });
    return reply.code(204).send();
  });

  app.post("/api/v1/import", async (request, reply) => {
    const input = ImportBody.parse(request.body);
    const organizationId = orgId(request);
    let imported = 0;

    for (const employee of input.employees) {
      await upsertEmployee(organizationId, employee, false, reply);
      imported += 1;
    }
    for (const shift of input.shifts) {
      await upsertShift(organizationId, shift, false, reply);
      imported += 1;
    }
    for (const assignment of input.assignments) {
      await upsertAssignment(organizationId, assignment, false, reply);
      imported += 1;
    }
    for (const roster of input.duty_rosters) {
      await upsertDutyRoster(organizationId, roster, false, reply);
      imported += 1;
    }
    for (const event of input.events) {
      const existing = await rows(
        pool,
        "SELECT id FROM time_events WHERE organization_id = $1 AND id = $2",
        [organizationId, event.id],
      );
      if (existing[0]) continue;
      await pool.query(
        `INSERT INTO time_events(id, organization_id, employee_id, event_type, occurred_at, occurred_at_epoch, source, note, device_id)
         VALUES ($1, $2, $3, $4, $5::timestamptz, $6, $7, $8, $9)`,
        [
          event.id,
          organizationId,
          event.employee_id,
          event.event_type,
          event.occurred_at,
          event.occurred_at_epoch,
          event.source,
          optionalText(event.note),
          deviceId(request),
        ],
      );
      await logChange(organizationId, "event", event.id, "upsert", event);
      imported += 1;
    }
    for (const [key, value] of Object.entries(input.settings)) {
      const existing = await rows(
        pool,
        "SELECT updated_at FROM settings WHERE organization_id = $1 AND key = $2",
        [organizationId, key],
      );
      if (existing[0]) continue;
      await pool.query(
        "INSERT INTO settings(organization_id, key, value) VALUES ($1, $2, $3)",
        [organizationId, key, value],
      );
      await logChange(organizationId, "setting", key, "upsert", { key, value });
      imported += 1;
    }

    return reply.send({ imported });
  });

  return app;
}
