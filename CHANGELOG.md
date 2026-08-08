# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

## [0.5.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.4.0..v0.5.0) - 2026-08-08

### Bug Fixes

- **(mcp)** protect recipes-file by exact path, only if inside - ([c28c92c](https://github.com/DevelAngel/matrix-mcp/commit/c28c92c326df83261439ca4db2eaa14e3328d00f)) - Angelos Drossos

### Documentation

- **(adr)** add ADR 0005 for per-recipe MCP tools - ([0131f5b](https://github.com/DevelAngel/matrix-mcp/commit/0131f5ba3f64ae5f9e04e177484fe875905f467a)) - Angelos Drossos
- add internal recipe interpreter ADR - ([77380ee](https://github.com/DevelAngel/matrix-mcp/commit/77380ee9a9a090c02eb58a57fd38148876b52fea)) - Angelos Drossos

### Features

- **(mcp)** add recipe_run tool alongside just_run - ([c39f97b](https://github.com/DevelAngel/matrix-mcp/commit/c39f97bce846381a03aa21dcdd155587b7038719)) - Angelos Drossos
- **(mcp)** replace recipe_run with per-recipe MCP tools - ([0ac5194](https://github.com/DevelAngel/matrix-mcp/commit/0ac519436efc52e1a92aadbf62fc3a24684b7c66)) - Angelos Drossos
- **(recipe)** add TOML recipe interpreter crate - ([974e23c](https://github.com/DevelAngel/matrix-mcp/commit/974e23c5e8555fb0f05db73222c0af54badc51ac)) - Angelos Drossos

### Miscellaneous Chores

- add recipes.toml for the recipe_run tool - ([e8e3650](https://github.com/DevelAngel/matrix-mcp/commit/e8e3650b46783a1b20fbae63d2918f2dd761fe2b)) - Angelos Drossos

## [0.4.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.3.0..v0.4.0) - 2026-08-06

### Bug Fixes

- **(mcp)** allow only one just recipe call - ([a2b8a30](https://github.com/DevelAngel/matrix-mcp/commit/a2b8a30df8168ea178c0dee52a5554994a13c8e2)) - Angelos Drossos

### Build

- **(prek)** add prepare commit msg hook - ([3d842c4](https://github.com/DevelAngel/matrix-mcp/commit/3d842c4c5a8a3f571a6c17e1e4dee1b03b21f82e)) - Angelos Drossos

### Documentation

- reduce src comments - ([39df30f](https://github.com/DevelAngel/matrix-mcp/commit/39df30f82b7b8bfa5f67c1cf7bc0eaebdf726865)) - Angelos Drossos

### Features

- **(mcp)** support just recipe arguments - ([8c0334a](https://github.com/DevelAngel/matrix-mcp/commit/8c0334a9a0e1e3642c8833cd53d6326a122bf34a)) - Angelos Drossos
- **(mcp)** inject just recipe argument help - ([7a9ea45](https://github.com/DevelAngel/matrix-mcp/commit/7a9ea4546bbae464960b2f3c8e2e23a507edb366)) - Angelos Drossos

### Miscellaneous Chores

- improve just recipe desc - ([7ff1fd2](https://github.com/DevelAngel/matrix-mcp/commit/7ff1fd29f149495c4d5d2633d703ca6bfafddde0)) - Angelos Drossos
- hide release-cross just recipe - ([d6731bb](https://github.com/DevelAngel/matrix-mcp/commit/d6731bb57077fdd308abe69f57d47cae54444f76)) - Angelos Drossos
- add git just recipes - ([f2bcc22](https://github.com/DevelAngel/matrix-mcp/commit/f2bcc22a5784031d4a3a2e58a55650f28cc2b5ef)) - Angelos Drossos

### Refactoring

- **(mcp)** use just --show for recipe descriptions - ([e7526e2](https://github.com/DevelAngel/matrix-mcp/commit/e7526e2cfd91dbfd5d65c85c76ca96cee7809b54)) - Angelos Drossos

## [0.3.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.2.0..v0.3.0) - 2026-08-05

### Documentation

- add sandboxed just tool ADR - ([f60d3dc](https://github.com/DevelAngel/matrix-mcp/commit/f60d3dc581dd8776e10ca6f7f4244069ba23b6b4)) - Angelos Drossos
- reduce src documentation - ([40f8ca8](https://github.com/DevelAngel/matrix-mcp/commit/40f8ca8472b7a300fa95a20ce680c8d484f3775a)) - Angelos Drossos

### Features

- add just_run tool - ([be4f1d1](https://github.com/DevelAngel/matrix-mcp/commit/be4f1d1c802429edf62a37c053195b3369568151)) - Angelos Drossos
- refuse to write justfile - ([23a19c7](https://github.com/DevelAngel/matrix-mcp/commit/23a19c796ec49c4a6d8b8e6bd40cbbb4a2898181)) - Angelos Drossos
- refuse to write justfile (BufWriter) - ([39e70bc](https://github.com/DevelAngel/matrix-mcp/commit/39e70bceb6333b9f4a4b8c8fda11c1cff5f21c76)) - Angelos Drossos
- inject just recipes into tool desc - ([bff994b](https://github.com/DevelAngel/matrix-mcp/commit/bff994b9ccdda807125c10d29281e66a36ce1a3f)) - Angelos Drossos
- inject just recipes incl. description - ([5345db5](https://github.com/DevelAngel/matrix-mcp/commit/5345db5343f4d76d0fef8083019e367c58c4ded7)) - Angelos Drossos
- add extra-ignore CLI option - ([bb33e52](https://github.com/DevelAngel/matrix-mcp/commit/bb33e52d0da998d2f311029630155bd40ef3e922)) - Angelos Drossos
- use glob pattern for ignore lists - ([5f2b118](https://github.com/DevelAngel/matrix-mcp/commit/5f2b118522a85d8e1abcdbd95fdbadf343b134c9)) - Angelos Drossos
- introduce enable-just-run CLI option - ([e4c82c9](https://github.com/DevelAngel/matrix-mcp/commit/e4c82c9e352ca5967493f21702f9aa45d3bee20e)) - Angelos Drossos

## [0.2.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.1.0..v0.2.0) - 2026-08-03

### Documentation

- add README - ([98e3b72](https://github.com/DevelAngel/matrix-mcp/commit/98e3b720a20dfb1c23302af0a1eb1f53dd2d219b)) - Angelos Drossos
- add workspace scoped paths ADR - ([0c251e3](https://github.com/DevelAngel/matrix-mcp/commit/0c251e333be9f7a42d9a0b28e966903c4b1cabbf)) - Angelos Drossos
- add ignore list ADR - ([04639e2](https://github.com/DevelAngel/matrix-mcp/commit/04639e268b33c69ea8514223df36c35b3cbc4079)) - Angelos Drossos

### Features

- add OAuth 2.1 authorization to MCP server - ([16771f1](https://github.com/DevelAngel/matrix-mcp/commit/16771f17b4b1db907f814e9fb46c849cf0c3cf0a)) - Angelos Drossos
- add ignore option - ([78ad9ab](https://github.com/DevelAngel/matrix-mcp/commit/78ad9ab9d04a0c7b68a0af38cb3efa0035980acb)) - Angelos Drossos

### Refactoring

- enforce workspace path safety through the type system - ([d874e2f](https://github.com/DevelAngel/matrix-mcp/commit/d874e2f09354aa3699d231a07b990cee8b01f3ef)) - Angelos Drossos

## [0.1.0] - 2026-08-01

### Build

- init kid-text-editor crate - ([80e8d8d](https://github.com/DevelAngel/matrix-mcp/commit/80e8d8db3d0add611d67c217809ab7c64de7865a)) - Angelos Drossos
- add just config - ([7682939](https://github.com/DevelAngel/matrix-mcp/commit/7682939ac1ea31b6ec43ccdd5c3758b9e0f6f9f0)) - Angelos Drossos
- add deno config - ([c294383](https://github.com/DevelAngel/matrix-mcp/commit/c294383e29e52ed93c76c94d368fd5c020a560b4)) - Angelos Drossos
- add clippy config - ([f75a0ed](https://github.com/DevelAngel/matrix-mcp/commit/f75a0ed9cad058c4eca52efe0d7e84305ff33030)) - Angelos Drossos
- add git-cliff config - ([779b960](https://github.com/DevelAngel/matrix-mcp/commit/779b960aff34df0aa48c9395b9c1322c2888ffd1)) - Angelos Drossos
- add prek config (pre-commit hook) - ([75c24e8](https://github.com/DevelAngel/matrix-mcp/commit/75c24e88ff06f3ef8fec1b78921c2cbc31de7c44)) - Angelos Drossos
- add unit tests - ([214d7d6](https://github.com/DevelAngel/matrix-mcp/commit/214d7d624ab92c21c75ebcd02aa37029d5f58a79)) - Angelos Drossos

### Features

- add mcp server with view tool - ([e278d8e](https://github.com/DevelAngel/matrix-mcp/commit/e278d8e43e6f28237dd4169242388e7668a8a7f4)) - Angelos Drossos
- add tree tool - ([fd80461](https://github.com/DevelAngel/matrix-mcp/commit/fd8046195697cdf6e637f35cc873ab9f2b91e4fe)) - Angelos Drossos
- add create tool - ([7463f05](https://github.com/DevelAngel/matrix-mcp/commit/7463f05ff6487fea6b60a021305361ff09dd6e2e)) - Angelos Drossos
- add insert tool - ([684e9d2](https://github.com/DevelAngel/matrix-mcp/commit/684e9d2e6a467f0ab6ebd600839db19cc4ff5782)) - Angelos Drossos
- add str-replace tool - ([88ba3bd](https://github.com/DevelAngel/matrix-mcp/commit/88ba3bdbfd80f2ca58e24c9c0467db0e742adabf)) - Angelos Drossos

<!-- generated by git-cliff -->
