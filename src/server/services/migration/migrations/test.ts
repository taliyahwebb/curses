import IMigration from "../IMigration";

export default {
  isApplicable: () => import.meta.env.DEV,

  isStillValid: (..._) => false,

  apply: async () => new Object(),
} as IMigration;
