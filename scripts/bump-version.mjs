// SPDX-License-Identifier: AGPL-3.0-or-later
// Update every release version reference for an explicit release PR.
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
const semver = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
if (!version || !semver.test(version)) {
	console.error("usage: node scripts/bump-version.mjs <major.minor.patch>");
	process.exit(1);
}

const root = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");
const write = (path, text) => writeFileSync(new URL(path, root), text);

const cargoToml = read("server/Cargo.toml");
const previous = cargoToml.match(/^version = "([^"]+)"/m)?.[1];
if (!previous) {
	console.error("version: server/Cargo.toml has no package version");
	process.exit(1);
}

write(
	"server/Cargo.toml",
	cargoToml.replace(/^version = "[^"]+"/m, `version = "${version}"`),
);

const cargoLock = read("server/Cargo.lock");
const nextLock = cargoLock.replace(
	/(\[\[package\]\]\nname = "cupful"\nversion = ")[^"]+"/,
	(match, prefix) => `${prefix}${version}"`,
);
if (nextLock === cargoLock) {
	console.error('version: server/Cargo.lock has no cupful package');
	process.exit(1);
}
write("server/Cargo.lock", nextLock);

const pkg = JSON.parse(read("web/package.json"));
pkg.version = version;
write("web/package.json", `${JSON.stringify(pkg, null, "\t")}\n`);

const packageLock = JSON.parse(read("web/package-lock.json"));
packageLock.version = version;
if (packageLock.packages?.[""]) packageLock.packages[""].version = version;
write("web/package-lock.json", `${JSON.stringify(packageLock, null, "\t")}\n`);

for (const path of ["README.md", "docs/self-hosting.md", "compose.yaml"]) {
	const text = read(path);
	if (!text.includes(previous)) {
		console.error(`version: ${path} does not mention ${previous}`);
		process.exit(1);
	}
	write(path, text.replaceAll(previous, version));
}

console.log(`Bumped ${previous} to ${version}`);
