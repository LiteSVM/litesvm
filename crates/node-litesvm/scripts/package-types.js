const { mkdirSync, writeFileSync } = require("node:fs");

const packageTypes = [
	["dist/cjs", "commonjs"],
	["dist/esm", "module"],
];

for (const [directory, type] of packageTypes) {
	mkdirSync(directory, { recursive: true });
	writeFileSync(`${directory}/package.json`, `{\n\t"type": "${type}"\n}\n`);
}
