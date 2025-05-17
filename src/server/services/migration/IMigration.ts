import { MigrationData } from "./types";

export default interface IMigration {
  /**
   * Checks if the migration is relevant to the user.
   * eg: a migration might be platform dependant.
   * An inapplicable migration will not be rerun, as if it had been applied.
   *
   * This function is also supposed to stop us from breaking something by applying the same
   * migration twice if the user deletes the migration tracking file.
   */
  isApplicable(): boolean;

  /**
   * Checks if the migration is still valid by looking at the data saved at the time of migration.
   * This can for example be used to re-run a migration after an update, or if some data changed.
   * @param version - the version the migration was performed in
   * @param data - the data return by `apply` the last time the migration took place
   * @returns - true if the migration doesn't have to be re-run, false if it does
   * Note: if this function returns false, the migration still has to be applicable to be run.
   */
  isStillValid(version: string, data: MigrationData): boolean;

  /**
   * Applies the migration.
   * This function should use as little imported code as possible in order to not break
   * anything if something it relies on gets changed throughout development,
   * even at the cost of "duplicate" code.
   * @returns - data used for future validation
   */
  apply(): Promise<MigrationData>;
}
