import { pathsToModuleNameMapper } from "ts-jest";
import { compilerOptions } from "./tsconfig.json";

export default {
	preset: "ts-jest",
	testEnvironment: "node",
	moduleDirectories: ["node_modules", "./litesvm"],
	moduleFileExtensions: ["js", "ts"],
	moduleNameMapper: pathsToModuleNameMapper(compilerOptions.paths, {
		prefix: "<rootDir>/litesvm",
	}),
	// don't run copyAccounts test by default since devnet is flaky
	testPathIgnorePatterns: [
		"<rootDir>/tests/util.ts",
		"<rootDir>/tests/copyAccounts.test.ts",
	],
	transform: {
		"^.+\\.{ts|tsx}?$": [
			"@swc/jest",
			{
				tsConfig: "tsconfig.json",
			},
		],
	},
};
