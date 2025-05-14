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
   * Applies the migration.
   * This function should use as little imported code as possible in order to not break
   * anything if something it relies on gets changed throughout development,
   * even at the cost of "duplicate" code.
   */
  apply(): Promise<void>;
}
