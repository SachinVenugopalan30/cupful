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
const escape = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const fail = (message) => {
	console.error(`version: ${message}`);
	process.exit(1);
};

const cargoToml = read("server/Cargo.toml");
const previous = cargoToml.match(/^version = "([^"]+)"/m)?.[1];
if (!previous) fail("server/Cargo.toml has no package version");
const nextCargoToml = cargoToml.replace(/^version = "[^"]+"/m, `version = "${version}"`);

const cargoLock = read("server/Cargo.lock");
const nextLock = cargoLock.replace(
	/(\[\[package\]\]\nname = "cupful"\nversion = ")[^"]+"/,
	(match, prefix) => `${prefix}${version}"`,
);
if (nextLock === cargoLock) fail("server/Cargo.lock has no cupful package");

const pkg = JSON.parse(read("web/package.json"));
pkg.version = version;

const packageLock = JSON.parse(read("web/package-lock.json"));
if (!packageLock.packages?.[""]) fail("web/package-lock.json has no root package");
packageLock.version = version;
packageLock.packages[""].version = version;

const replacements = [
	[new RegExp(`Version ${escape(previous)}\\.`, "g"), `Version ${version}.`],
	[new RegExp(`cupful:${escape(previous)}(?![\\d.-])`, "g"), `cupful:${version}`],
	[new RegExp(`\`${escape(previous)}\``, "g"), `\`${version}\``],
];
const docs = new Map();
for (const path of ["README.md", "docs/self-hosting.md", "compose.yaml"]) {
	const text = read(path);
	let next = text;
	for (const [from, to] of replacements) next = next.replaceAll(from, to);
	if (next === text) fail(`${path} does not mention ${previous}`);
	docs.set(path, next);
}

write("server/Cargo.toml", nextCargoToml);
write("server/Cargo.lock", nextLock);
write("web/package.json", `${JSON.stringify(pkg, null, "\t")}\n`);
write("web/package-lock.json", `${JSON.stringify(packageLock, null, "\t")}\n`);
for (const [path, text] of docs) write(path, text);

console.log(`Bumped ${previous} to ${version}`);
