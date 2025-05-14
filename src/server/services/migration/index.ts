import { exists, readFile, writeFile, BaseDirectory } from "@tauri-apps/plugin-fs";
import IMigration from "./IMigration";
import migrations from "./migrations";

type MigrationID = keyof typeof migrations;

/**
 * Get applied migrations saved in `$LOCALAPPDATA\{app identifier}\migrations.json`.
 * @returns - list of migration file basenames
 */
async function getAppliedMigrationIDs(): Promise<MigrationID[]> {
  const MIGRATIONS_FILE = ["migrations.json", { baseDir: BaseDirectory.AppLocalData }] as const;

  if (!await exists(...MIGRATIONS_FILE))
    return [];

  const json = new TextDecoder().decode(await readFile(...MIGRATIONS_FILE));

  const migrationIDs = JSON.parse(json);
  if (!Array.isArray(migrationIDs))
    return [];

  return migrationIDs;
}

/**
 * Applies a migration if applicable.
 * @param migrationID - migration file basename
 */
function apply(migrationID: MigrationID) {
  const migration: IMigration = migrations[migrationID];

  if (migration.isApplicable()) {
    console.log(`Migration \`${migrationID}\`: applied`);
    migration.apply();
  } else
    console.log(`Migration \`${migrationID}\`: skipped`);
}

/**
 * Tries to apply every new migration.
 */
export default async function applyAllMigrations() {
  const migrationIDs = <MigrationID[]>Object.keys(migrations);
  const applied = await getAppliedMigrationIDs();

  const migrationsToApply = migrationIDs.filter(migrationID => !applied.includes(migrationID));

  for (const migrationID of migrationsToApply) {
    try {
      apply(migrationID);
      applied.push(migrationID);
    } catch (err) {
      console.error(err)
    }
  }

  const json = new TextEncoder().encode(JSON.stringify(migrationIDs, null, 4));
  writeFile("migrations.json", json, { baseDir: BaseDirectory.AppLocalData });
}
