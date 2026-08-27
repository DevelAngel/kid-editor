# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

## [0.11.1](https://github.com/DevelAngel/matrix-mcp/compare/v0.11.0..v0.11.1) - 2026-08-27

### Bug Fixes

- **(mcp)** enforce single-line replacement - ([fb605ca](https://github.com/DevelAngel/matrix-mcp/commit/fb605ca8c411ef1775af20262060bd9ebb69e39d)) - Angelos Drossos

### Style

- **(mcp)** use std assert_matches import - ([1cb400c](https://github.com/DevelAngel/matrix-mcp/commit/1cb400c872ea1445d604b38f56c64b35b825409e)) - Angelos Drossos
- **(mcp)** use assert_matches imports - ([70bdfb5](https://github.com/DevelAngel/matrix-mcp/commit/70bdfb584df8197225fdcb9988aa15251f47837b)) - Angelos Drossos
- **(mcp)** use error code imports - ([f473251](https://github.com/DevelAngel/matrix-mcp/commit/f473251b3f1eee3be57e878bd58272bf90334468)) - Angelos Drossos

### Tests

- **(mcp)** assert replacement error type - ([fbcdb3d](https://github.com/DevelAngel/matrix-mcp/commit/fbcdb3d1cb0bc1c76cfa618b09640e5aaa7e126a)) - Angelos Drossos
- **(mcp)** assert tool error types - ([73486a5](https://github.com/DevelAngel/matrix-mcp/commit/73486a5912befeae7ba8fd884c973c955260d0b8)) - Angelos Drossos
- **(mcp)** assert glob parse errors - ([c39314d](https://github.com/DevelAngel/matrix-mcp/commit/c39314dd7193bc37c240943148e4a887146081ba)) - Angelos Drossos

## [0.11.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.10.0..v0.11.0) - 2026-08-21

### Bug Fixes

- **(gateway)** log full args/output at right levels - ([72fd14c](https://github.com/DevelAngel/matrix-mcp/commit/72fd14c70f011fcdbbaf9c01ecdf1880c95aeda1)) - Angelos Drossos
- **(recipes)** use gh-monitor for review comments - ([f03c0ac](https://github.com/DevelAngel/matrix-mcp/commit/f03c0ac1d33540a6c4ff5a14e321ae49e47dd4ba)) - Angelos Drossos
- **(text)** log full args/output at right levels - ([f20ee1e](https://github.com/DevelAngel/matrix-mcp/commit/f20ee1e253f138b72adeec633a3f2c01fc6dc5ff)) - Angelos Drossos

### Documentation

- add video - ([7599337](https://github.com/DevelAngel/matrix-mcp/commit/75993377f88767644b377f11068face8ab3d06ec)) - Angelos Drossos

### Features

- **(mcp)** [**breaking**] make fs_replace_lines single-line only - ([098dc2c](https://github.com/DevelAngel/matrix-mcp/commit/098dc2c9ce3bfbafcc939d6e3718e73f9ab7d279)) - Angelos Drossos
- **(mcp)** log recipe parameters per configured level - ([fc4393c](https://github.com/DevelAngel/matrix-mcp/commit/fc4393c606bdf94ea2943eb7ad19b3781e3d2aa0)) - Angelos Drossos
- **(recipe)** add per-parameter log level and kind - ([e904e9b](https://github.com/DevelAngel/matrix-mcp/commit/e904e9b81d2a340c376f714cd1a631ca57164f11)) - Angelos Drossos
- **(recipes)** set log level and kind on parameters - ([0cb4d2e](https://github.com/DevelAngel/matrix-mcp/commit/0cb4d2ea03e02ba5aa2addc238648f7907193fab)) - Angelos Drossos
- **(text)** return edited excerpt instead of a bare summary - ([79f9049](https://github.com/DevelAngel/matrix-mcp/commit/79f9049e1aefb1444eb46ba7ec0d0ddf16822a26)) - Angelos Drossos

### Refactoring

- **(mcp)** [**breaking**] rename fs_replace_lines to fs_replace_line - ([551b23d](https://github.com/DevelAngel/matrix-mcp/commit/551b23df48015d75088bd0dbf883b627fd51e02b)) - Angelos Drossos
- **(recipe)** trim doc comments, fold formatting into Formatted - ([297d53e](https://github.com/DevelAngel/matrix-mcp/commit/297d53e0c3ccf3c2113f858a82be0d642f4383b4)) - Angelos Drossos
- **(text)** trim render.rs doc comments to essentials - ([53fb1f5](https://github.com/DevelAngel/matrix-mcp/commit/53fb1f5059d9418ebc4eb62baba8eb182e79dfdd)) - Angelos Drossos

## [0.10.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.9.0..v0.10.0) - 2026-08-16

### Bug Fixes

- **(mcp)** [**breaking**] fs_create refuses to overwrite existing files - ([44841b7](https://github.com/DevelAngel/matrix-mcp/commit/44841b7bf9ee1099b6cfcdb7bcb263faa07f82ed)) - Angelos Drossos
- **(mcp)** shorten fs_create descriptions - ([fe33a00](https://github.com/DevelAngel/matrix-mcp/commit/fe33a0033e4ad466bf4ed6e83935550fdcf724ab)) - Angelos Drossos
- **(recipe)** restore real per-recipe --help via mut_subcommand - ([e82f85b](https://github.com/DevelAngel/matrix-mcp/commit/e82f85b7c5c539992a1545057edad79469c14720)) - Angelos Drossos
- **(recipes)** prefix all git recipe titles with "Git" - ([efb63c4](https://github.com/DevelAngel/matrix-mcp/commit/efb63c44ec961ce338ff4020779fdb650f4df792)) - Angelos Drossos

### Build

- **(recipe)** [**breaking**] rename binary to kid-recipes - ([716d30b](https://github.com/DevelAngel/matrix-mcp/commit/716d30b43da82d476d91e6efa093b5c41f47623d)) - Angelos Drossos
- **(recipes)** prefix rust recipes - ([2a98428](https://github.com/DevelAngel/matrix-mcp/commit/2a98428829078ffc33606b7401b5c6acaaef948d)) - ”Navi-KID”
- update dependencies - ([738745b](https://github.com/DevelAngel/matrix-mcp/commit/738745b91db98af819b452c019a327d7aecfe5d6)) - ”Navi-KID”

### Documentation

- improve README - ([1cdf253](https://github.com/DevelAngel/matrix-mcp/commit/1cdf253120dd2538ec04dfc99097055a298e102d)) - Angelos Drossos

### Features

- **(recipe)** [**breaking**] switch CLI to clap builder API - ([2f9a7f2](https://github.com/DevelAngel/matrix-mcp/commit/2f9a7f2e3c7a4a1d89b9de159dab150a205e13d6)) - Angelos Drossos
- **(recipe)** [**breaking**] drop list, run --help already enumerates recipes - ([8a553c0](https://github.com/DevelAngel/matrix-mcp/commit/8a553c0de8e77b03c96eabd21bfe867b0ede301f)) - Angelos Drossos
- **(recipe)** [**breaking**] drop run prefix, recipes are top-level subcommands - ([513394d](https://github.com/DevelAngel/matrix-mcp/commit/513394d5ece10f5ca5d03ec1787648113a6dabf1)) - Angelos Drossos
- **(recipe)** resolve --file/--cwd via clap, not manual scanning - ([aba4411](https://github.com/DevelAngel/matrix-mcp/commit/aba4411bf4a0954c48f29ca3a17142a5fd06d183)) - Angelos Drossos
- **(recipes)** add tool titles to recipes.toml - ([2e0fb8e](https://github.com/DevelAngel/matrix-mcp/commit/2e0fb8e3aa1bf0775bf3e3b6193fb7b0a8b0f389)) - Angelos Drossos

### Refactoring

- **(recipe)** back CLI with derive API, keep builder minimal - ([ffe1189](https://github.com/DevelAngel/matrix-mcp/commit/ffe1189fe7dde9026eaa9f14cadda82a41213a3a)) - Angelos Drossos
- **(recipe)** start from Cli::parse(), drop manual pre-scan - ([10e197e](https://github.com/DevelAngel/matrix-mcp/commit/10e197e5bb9c867e376bcd0d09f75ac62fcb2432)) - Angelos Drossos
- **(recipe)** use anyhow for our own error paths - ([d19c1f7](https://github.com/DevelAngel/matrix-mcp/commit/d19c1f7bd457c7bb3ad6c64c57f7208e06d5a1ab)) - Angelos Drossos
- **(recipe)** main returns Result<()> directly - ([c347f0c](https://github.com/DevelAngel/matrix-mcp/commit/c347f0cd3bd741694c99c11fc577796d49f43b87)) - Angelos Drossos
- **(recipe)** augment_with_recipes as a Command extension trait - ([a7abe91](https://github.com/DevelAngel/matrix-mcp/commit/a7abe91e900d338ca63fe37f77769f84eaa56e10)) - Angelos Drossos

### Style

- **(recipe)** unqualify process::exit, note Result<!> is unstable - ([aeb24b9](https://github.com/DevelAngel/matrix-mcp/commit/aeb24b97cc8956e50030926072b1c74313dbfa79)) - Angelos Drossos
- **(recipe)** trim AugmentWithRecipes doc comments - ([50b3baf](https://github.com/DevelAngel/matrix-mcp/commit/50b3bafe03e0642032b3e3e193a899ab2a125ad5)) - Angelos Drossos

### Tests

- **(recipe-run)** fix renamed recipe names in sanity test - ([b6ea567](https://github.com/DevelAngel/matrix-mcp/commit/b6ea567d80f09a00f6d5aa345e26c69a068b62bb)) - Angelos Drossos

## [0.9.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.8.0..v0.9.0) - 2026-08-14

### Bug Fixes

- **(mcp)** rename fs_view title to cover dir listing - ([77970ca](https://github.com/DevelAngel/matrix-mcp/commit/77970caf09f1b99bb0b42fce5c064fbb782fbfcc)) - Angelos Drossos
- **(recipes)** reference in-repo skill, not local one - ([d53d482](https://github.com/DevelAngel/matrix-mcp/commit/d53d482f8a85de51dfce9e5964af2090db239079)) - Angelos Drossos
- extract real text from CallToolResponse for logging - ([379bfd5](https://github.com/DevelAngel/matrix-mcp/commit/379bfd532a28ae88c06ab89e1bbbf15a727eeb40)) - Angelos Drossos

### Documentation

- **(mcp)** note missing recipe_* tool annotations - ([0b48cad](https://github.com/DevelAngel/matrix-mcp/commit/0b48cad4b5d28e6df544ebe664c98cd2bd6fe601)) - Angelos Drossos
- **(recipe)** shorten RecipeAnnotations doc comment - ([4984367](https://github.com/DevelAngel/matrix-mcp/commit/498436738e6db55d0b66b6ceec4c8afce6b0d661)) - Angelos Drossos
- **(recipe)** drop cross-file pointer from RecipeAnnotations - ([5bebdf2](https://github.com/DevelAngel/matrix-mcp/commit/5bebdf2262f0f20fd31dd03daf18b185a73ae673)) - Angelos Drossos
- **(skills)** add code-review-request-writing-guide - ([6e3e0f1](https://github.com/DevelAngel/matrix-mcp/commit/6e3e0f179fef9ba210e25b6bc9b841631ce16962)) - Angelos Drossos
- note fs_str_replace removal in README - ([d821ef1](https://github.com/DevelAngel/matrix-mcp/commit/d821ef1b8ba496602f88283d52553f500133699b)) - Angelos Drossos

### Features

- **(gateway)** log tool calls with path and size - ([f257309](https://github.com/DevelAngel/matrix-mcp/commit/f25730901a94c74dc0e0632e89ae5fa2ebd6b6d0)) - Angelos Drossos
- **(mcp)** add annotations to static fs tools - ([777e3f3](https://github.com/DevelAngel/matrix-mcp/commit/777e3f35216d2b503b6dba5cd6b8a2e946513be3)) - Angelos Drossos
- **(mcp)** wire recipe annotations into generated tools - ([c966983](https://github.com/DevelAngel/matrix-mcp/commit/c9669830e60466a4920bd28bb05573ff681b601b)) - Angelos Drossos
- **(recipe)** add optional annotation fields to Recipe - ([e7186bd](https://github.com/DevelAngel/matrix-mcp/commit/e7186bd0423432cfb875c54cdca5811d0b1e8ca4)) - Angelos Drossos
- **(text)** log tool calls with path and size - ([0878c61](https://github.com/DevelAngel/matrix-mcp/commit/0878c6139872f9d2346c726d0ccfd40bf46ffe7b)) - Angelos Drossos
- **(tools)** add line-addressed edit tools - ([ab8da81](https://github.com/DevelAngel/matrix-mcp/commit/ab8da8197fb4ea673652973102c2a60f2498d0a1)) - Angelos Drossos
- add --log-baseline, verbosity applies repo-wide - ([81214b9](https://github.com/DevelAngel/matrix-mcp/commit/81214b9804e191fd201676e84753feb3ea22f64e)) - Angelos Drossos

### Miscellaneous Chores

- **(recipes)** declare annotations for existing recipes - ([fe0d1c9](https://github.com/DevelAngel/matrix-mcp/commit/fe0d1c9032d83272f06d1832cb81c9bad7da3137)) - Angelos Drossos
- **(recipes)** point gh-pr-create/-edit at mr-writing-guide - ([f05752e](https://github.com/DevelAngel/matrix-mcp/commit/f05752efe97903f357059415f917291aeccdbfd6)) - Angelos Drossos

### Refactoring

- **(tools)** replace line-address free fns with types - ([6339786](https://github.com/DevelAngel/matrix-mcp/commit/633978634108424f636e85412f008ce2e3f4ffc2)) - Angelos Drossos
- **(tools)** [**breaking**] remove fs_str_replace and fs_insert - ([82d0ecd](https://github.com/DevelAngel/matrix-mcp/commit/82d0ecd50559959e07d87b8d4564f100985f6d7a)) - Angelos Drossos
- build EnvFilter from typed Directives - ([375623e](https://github.com/DevelAngel/matrix-mcp/commit/375623e228dfff13f1716f9eb204f235dc308d08)) - Angelos Drossos
- turn output_text into an extension trait method - ([7982d3d](https://github.com/DevelAngel/matrix-mcp/commit/7982d3d1d1858a81940e6ccb648f4290a3b5cba3)) - Angelos Drossos
- extract kid-logging crate for shared env_filter - ([25c4ca5](https://github.com/DevelAngel/matrix-mcp/commit/25c4ca5b8dc115dab24444d83ae3b3f4fdbca7cf)) - Angelos Drossos
- list only bin crates as workspace members - ([ac717f6](https://github.com/DevelAngel/matrix-mcp/commit/ac717f63006a50d0b897dc56bbe079137b1bed3b)) - Angelos Drossos

### Style

- **(recipes)** reference skill by name, not path - ([b3e8d2d](https://github.com/DevelAngel/matrix-mcp/commit/b3e8d2da5a63f141cddefd09e4436de92770d42d)) - Angelos Drossos

### Tests

- **(gateway)** assert prefixed() preserves annotations - ([c6025fd](https://github.com/DevelAngel/matrix-mcp/commit/c6025fd1d2c731af9f21e1be2b93934ba918f8c9)) - Angelos Drossos

## [0.8.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.7.0..v0.8.0) - 2026-08-12

### Bug Fixes

- **(server)** disable legacy MCP session mode - ([f17c3a8](https://github.com/DevelAngel/matrix-mcp/commit/f17c3a8f53fdb31b7211c93b177f7971b6ca8c25)) - Angelos Drossos
- prefix oauth package name with kid- - ([5e3219a](https://github.com/DevelAngel/matrix-mcp/commit/5e3219ae985bb300b2511a300d4b119b69f2513c)) - Angelos Drossos
- prefix recipe package name with kid- - ([1494942](https://github.com/DevelAngel/matrix-mcp/commit/14949423b97dcf7b4e1ab0242562d2017cd3f224)) - Angelos Drossos
- address review feedback on oauth features - ([5464e16](https://github.com/DevelAngel/matrix-mcp/commit/5464e16a921e131b8f9b3afc837752cb54df68f7)) - Angelos Drossos

### Features

- **(gateway)** scaffold kid-mcp-gateway crate - ([28c8a0e](https://github.com/DevelAngel/matrix-mcp/commit/28c8a0eb7237bdaa9c175610005d757b60864782)) - Angelos Drossos
- **(gateway)** aggregate and route upstream tools - ([accb81e](https://github.com/DevelAngel/matrix-mcp/commit/accb81e31448878b0a68604fc5a816bed47927f0)) - Angelos Drossos
- **(oauth)** gate server code behind server feature - ([fbe118f](https://github.com/DevelAngel/matrix-mcp/commit/fbe118f992e2d686e8880c45afd439d1031fe62d)) - Angelos Drossos
- **(oauth)** add client-credentials helper - ([7f2b889](https://github.com/DevelAngel/matrix-mcp/commit/7f2b889c2e4a0a76aa62f43d2bca8f5f1f007680)) - Angelos Drossos

### Miscellaneous Chores

- **(recipes)** add gh-pr-approve recipe - ([f684161](https://github.com/DevelAngel/matrix-mcp/commit/f684161ed6f9fbf85cd08bd9efd0195c37dac144)) - Angelos Drossos

### Refactoring

- **(gateway)** key upstreams by name in an IndexMap - ([c6c1a98](https://github.com/DevelAngel/matrix-mcp/commit/c6c1a980b184b9ee4c8940597895950547fdbe59)) - Angelos Drossos
- **(gateway)** drop unnecessary Vec collect in connect - ([ed32ca0](https://github.com/DevelAngel/matrix-mcp/commit/ed32ca08d651ba0247e985fa1254c00db5f5c8dd)) - Angelos Drossos
- **(gateway)** fold tool lists into per-upstream storage - ([ccf46a9](https://github.com/DevelAngel/matrix-mcp/commit/ccf46a96ed0612529339a8561f651e628383cbe3)) - Angelos Drossos
- extract oauth crate from text - ([1c3dcc5](https://github.com/DevelAngel/matrix-mcp/commit/1c3dcc5c0f92c4d5e5e3375ace1cc6e32f8d0ae1)) - Angelos Drossos

## [0.7.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.6.0..v0.7.0) - 2026-08-10

### Bug Fixes

- **(recipes)** set upstream on git-push - ([5cfd925](https://github.com/DevelAngel/matrix-mcp/commit/5cfd925f8488323a7d3b834b02b8c28d899d7065)) - Angelos Drossos

### Documentation

- **(adr)** drop unneeded ADR 0006 and README notes - ([4b4b3f5](https://github.com/DevelAngel/matrix-mcp/commit/4b4b3f521e623af747dd3cd22b611254eb99fb0d)) - Angelos Drossos
- **(recipes)** add TODOs for hardcoded identity/repo values - ([c6f47f0](https://github.com/DevelAngel/matrix-mcp/commit/c6f47f081f08b14f2f9d1eada1bb4cbee27dc305)) - Angelos Drossos

### Features

- **(search)** add exact/case-insensitive search via grep-matcher - ([bba72e3](https://github.com/DevelAngel/matrix-mcp/commit/bba72e3a5540181781922cfed2b37bcec0358940)) - Angelos Drossos
- **(search)** add fuzzy search via nucleo - ([26c0e9c](https://github.com/DevelAngel/matrix-mcp/commit/26c0e9c649be8a3e8eedf16efe10708c019b9f84)) - Angelos Drossos

### Miscellaneous Chores

- **(recipes)** add git-restore recipe - ([edc0015](https://github.com/DevelAngel/matrix-mcp/commit/edc00158e5dca502775d21112bcf9d12a091dff6)) - Angelos Drossos
- **(recipes)** add git-branch recipe - ([eda7bfe](https://github.com/DevelAngel/matrix-mcp/commit/eda7bfe4bb6fa9861b2f32555869d97246c40bce)) - Angelos Drossos
- **(recipes)** filter review-comment payload via jq - ([6a160a1](https://github.com/DevelAngel/matrix-mcp/commit/6a160a1874cbebc32a5de2c89dbc07e2f6185a26)) - Angelos Drossos
- **(recipes)** add gh-pr-edit recipe - ([db91e68](https://github.com/DevelAngel/matrix-mcp/commit/db91e68a16c37650609ea4df60d79c6dca5e2f41)) - Angelos Drossos
- **(recipes)** add git-reset-soft recipe - ([bc11bfc](https://github.com/DevelAngel/matrix-mcp/commit/bc11bfc7e06a7b463559144e1b3ccf484741ea1f)) - Angelos Drossos
- **(recipes)** add git-push-force recipe - ([f5bf94b](https://github.com/DevelAngel/matrix-mcp/commit/f5bf94b8e1b4da6eee076f3096a477a65464a0d4)) - Angelos Drossos
- **(recipes)** add git-switch recipe - ([5a16fb6](https://github.com/DevelAngel/matrix-mcp/commit/5a16fb6e793957328ab779f4551f0fcf9c2e0584)) - Angelos Drossos
- **(recipes)** add git-pull recipe - ([f03420e](https://github.com/DevelAngel/matrix-mcp/commit/f03420e1f544bf1f6e767d0d17a1eb7862f52339)) - Angelos Drossos
- **(recipes)** add git-mv recipe - ([99a5044](https://github.com/DevelAngel/matrix-mcp/commit/99a5044db8372c2387f26c029d36e824c13f61ba)) - Angelos Drossos
- **(recipes)** add git-rm recipe - ([4f3c84c](https://github.com/DevelAngel/matrix-mcp/commit/4f3c84c0afba05809b4f33a8066f16f0e1142f4d)) - Angelos Drossos
- ignore semantic search index cache dir - ([63fa85a](https://github.com/DevelAngel/matrix-mcp/commit/63fa85afd5fc2159635c3ce6a98965be9f42d644)) - Angelos Drossos

### Refactoring

- **(mcp)** [**breaking**] prefix filesystem tools with fs_ - ([63131c0](https://github.com/DevelAngel/matrix-mcp/commit/63131c0f2a1110cbb342b3e25ee8041f70a22bf4)) - Angelos Drossos
- **(search)** apply review feedback - ([39c4de3](https://github.com/DevelAngel/matrix-mcp/commit/39c4de30ba5b910983af64e388fffa6647413935)) - Angelos Drossos
- **(search)** split into exact/fuzzy submodules - ([69bd0e5](https://github.com/DevelAngel/matrix-mcp/commit/69bd0e5a604684d3af5f3154afe75654c5b6a9a8)) - Angelos Drossos
- **(search)** thread WorkspacePath through the walk - ([791e2f1](https://github.com/DevelAngel/matrix-mcp/commit/791e2f119227a1444749d6fffdbcbbd8b1b402c7)) - Angelos Drossos
- **(workspace-path)** generalize child over AsRef<OsStr> - ([62aed2a](https://github.com/DevelAngel/matrix-mcp/commit/62aed2aae26873af88021a8c0c2aa58a589ad0e6)) - Angelos Drossos

### Style

- **(workspace-path)** hoist OsStr into the use block - ([33a7d60](https://github.com/DevelAngel/matrix-mcp/commit/33a7d602ab6edd0427c56bc7face044ee511791c)) - Angelos Drossos

## [0.6.0](https://github.com/DevelAngel/matrix-mcp/compare/v0.5.0..v0.6.0) - 2026-08-08

### Miscellaneous Chores

- remove unused justfile and helper.rs - ([ccdf6af](https://github.com/DevelAngel/matrix-mcp/commit/ccdf6afd4188e2fe9acbe9a48221e72f34825b60)) - Angelos Drossos

### Refactoring

- **(mcp)** [**breaking**] remove just_run tool - ([842cc3f](https://github.com/DevelAngel/matrix-mcp/commit/842cc3fb05f927944c59821bb15b2a9cc933b5a1)) - Angelos Drossos

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
