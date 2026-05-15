"use strict";

const { spawnSync } = require("node:child_process");

const PLATFORM_PACKAGES = {
  "linux:arm64": "@vitaly-zdanevich/rnsync-linux-arm64",
  "linux:x64": "@vitaly-zdanevich/rnsync-linux-x64",
};

function platformKey() {
  return `${process.platform}:${process.arch}`;
}

function binaryPackageName() {
  return PLATFORM_PACKAGES[platformKey()];
}

function resolveBinary(command) {
  const packageName = binaryPackageName();
  if (!packageName) {
    throw new Error(
      `rnsync npm binaries are not available for ${process.platform} ${process.arch}.`
    );
  }

  try {
    return require.resolve(`${packageName}/bin/${command}`);
  } catch (error) {
    if (error.code === "MODULE_NOT_FOUND") {
      throw new Error(
        `Could not find ${packageName}. Reinstall with optional dependencies enabled.`
      );
    }
    throw error;
  }
}

function run(command) {
  let binary;
  try {
    binary = resolveBinary(command);
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }

  const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if (result.signal) {
    console.error(`${command} exited from signal ${result.signal}`);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

module.exports = run;
module.exports.binaryPackageName = binaryPackageName;
module.exports.resolveBinary = resolveBinary;
