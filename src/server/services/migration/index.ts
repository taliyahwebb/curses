import { exists, readFile, writeFile, BaseDirectory } from "@tauri-apps/plugin-fs";
import { getVersion } from "@tauri-apps/api/app";
import IMigration from "./IMigration";
import { MigrationID, MigrationData, AppliedMigration } from "./types";
import migrations from "./migrations";

/**
 * Validate the contents of the migration tracking file
 * @param appliedMigrations - the contents of the migration tracking file
 */
function isValidMigrationsJSON(
  appliedMigrations: unknown,
): appliedMigrations is AppliedMigration[] {
  return (
    appliedMigrations &&
    Array.isArray(appliedMigrations) &&
    appliedMigrations.forEach((appliedMigration) => (
      "migrationID" in appliedMigration &&
      typeof appliedMigration.migrationID === "string" &&
      Object.keys(migrations).includes(appliedMigration.migrationID) &&
      "version" in appliedMigration &&
      typeof appliedMigration.version === "string" &&
      "data" in appliedMigration &&
      typeof appliedMigration.data === "object"
    ))
  ) as boolean;
}

/**
 * Get applied migrations saved in `$LOCALAPPDATA\{app identifier}\migrations.json`.
 * @returns - list of applied migrations
 */
async function getAppliedMigrations(): Promise<AppliedMigration[]> {
  const MIGRATIONS_FILE = ["migrations.json", { baseDir: BaseDirectory.AppLocalData }] as const;

  if (!(await exists(...MIGRATIONS_FILE))) return [];

  const json = new TextDecoder().decode(await readFile(...MIGRATIONS_FILE));

  const migrationsJSON = JSON.parse(json);
  if (!isValidMigrationsJSON(migrationsJSON)) return [];

  return migrationsJSON;
}

/**
 * Applies a migration if applicable.
 * @returns - the additional data for future validation
 */
async function apply(migrationID: MigrationID): Promise<MigrationData> {
  const migration: IMigration = migrations[migrationID];

  if (migration.isApplicable()) {
    console.log(`Migration \`${migrationID}\`: applied`);
    return await migration.apply();
  }
  console.log(`Migration \`${migrationID}\`: skipped`);
  return {};
}

/**
 * Applies an already-applied migration if its information shows it's out of date
 * @returns - a boolean indicating if the migration was rerun,
 * and the data associated to the last migration
 */
async function reapply(appliedMigration: AppliedMigration): Promise<[boolean, MigrationData]> {
  const migration: IMigration = migrations[appliedMigration.migrationID];

  if (migration.isStillValid(appliedMigration.version, appliedMigration.data))
    return [true, await apply(appliedMigration.migrationID)];
  return [false, appliedMigration.data];
}

/**
 * Tries to apply every new migration.
 */
export default async function applyAllMigrations() {
  const version = await getVersion();

  const migrationIDs = <MigrationID[]>Object.keys(migrations);
  const appliedMigrations = await getAppliedMigrations();

  const appliedMigrationIDs = appliedMigrations.map(migration => migration.migrationID);

  for (const migrationID of migrationIDs) {
    const appliedMigrationIndex = appliedMigrations.reduce(
        (lastIndex, appliedMigration, index) =>
          appliedMigration.migrationID === migrationID ? index : lastIndex,
        -1,
    );

    if (appliedMigrationIndex === -1) {
      let data: MigrationData;
      try {
        data = await apply(migrationID);
      } catch (err) {
        console.error(err);
        continue;
      }

      appliedMigrations.push({migrationID, version, data});
      continue;
    }

    // Applied migration
    let update: boolean, data: MigrationData;
    try {
      [update, data] = await reapply(appliedMigrations[appliedMigrationIndex]);
    } catch (err) {
      console.error(err);
      continue;
    }

    if (update) {
      appliedMigrations[appliedMigrationIndex] = {migrationID, version, data};
    }
  }

  const json = new TextEncoder().encode(JSON.stringify(migrationIDs, null, 4));
  writeFile("migrations.json", json, { baseDir: BaseDirectory.AppLocalData });
}
