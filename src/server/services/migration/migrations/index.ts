import IMigration from "../IMigration";
import test from "./test";
import grantMicAccess from "./grantMicAccess";
import devGrantMicAccess from "./devGrantMicAccess";

export default {
    test,
    grantMicAccess,
    devGrantMicAccess,
} as const;
