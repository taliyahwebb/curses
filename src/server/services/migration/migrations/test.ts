import IMigration from "../IMigration";

export default {
  isApplicable: () => import.meta.env.DEV,

  apply: async () => {},
} as IMigration;
