import migrations from "./migrations";

export type MigrationID = keyof typeof migrations;

export type MigrationData = {
  [key: string]:
    | string
    | number
    | MigrationData
    | string[]
    | number[]
    | MigrationData[];
};

export interface AppliedMigration {
  migrationID: MigrationID;
  version: string;
  data: MigrationData;
}
