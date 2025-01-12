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
	transform: {
		"^.+\\.{ts|tsx}?$": [
			"@swc/jest",
			{
				tsConfig: "tsconfig.json",
			},
		],
	},
};
