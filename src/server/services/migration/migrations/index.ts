import IMigration from "../IMigration";
import test from "./test";
import grantMicAccess from "./grantMicAccess";
import devGrantMicAccess from "./devGrantMicAccess";
import renameNative from "./renameNative";

export default {
    test,
    grantMicAccess,
    devGrantMicAccess,
    renameNative,
} as const;
