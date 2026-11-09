// SPDX-License-Identifier: AGPL-3.0-or-later
// Verify every release version reference agrees. Prints the version on success.
import { readFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");

const errors = [];
const check = (ok, message) => {
	if (!ok) errors.push(message);
};

const semver = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const cargoToml = read("server/Cargo.toml");
const version = cargoToml.match(/^version = "([^"]+)"/m)?.[1];

check(Boolean(version), "server/Cargo.toml has a package version");
if (!version) {
	console.error("version: server/Cargo.toml has no package version");
	process.exit(1);
}
check(semver.test(version), `server/Cargo.toml version ${version} is semver`);

const pkg = JSON.parse(read("web/package.json"));
check(pkg.version === version, `web/package.json is ${pkg.version}, expected ${version}`);

const packageLock = JSON.parse(read("web/package-lock.json"));
check(
	packageLock.version === version,
	`web/package-lock.json is ${packageLock.version}, expected ${version}`,
);
check(
	packageLock.packages?.[""]?.version === version,
	`web/package-lock.json root package is ${packageLock.packages?.[""]?.version}, expected ${version}`,
);

const cargoLock = read("server/Cargo.lock");
const cargoLockVersion = cargoLock.match(
	/\[\[package\]\]\nname = "cupful"\nversion = "([^"]+)"/,
)?.[1];
check(
	cargoLockVersion === version,
	`server/Cargo.lock package is ${cargoLockVersion}, expected ${version}`,
);

const readme = read("README.md");
check(readme.includes(`Version ${version}.`), `README.md does not say Version ${version}.`);
check(readme.includes(`:${version}`), `README.md does not use the ${version} image tag`);

const selfHosting = read("docs/self-hosting.md");
check(
	selfHosting.includes(`:${version}`),
	`docs/self-hosting.md does not use the ${version} image tag`,
);

const compose = read("compose.yaml");
check(compose.includes(`:${version}`), `compose.yaml does not use the ${version} image tag`);

if (errors.length > 0) {
	for (const error of errors) console.error(`version: ${error}`);
	process.exit(1);
}

console.log(version);
