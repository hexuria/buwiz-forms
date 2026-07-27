# Commit SHA remap — 2026-07-28 history rewrite

On 2026-07-28 the `main` branch was rewritten to remove `Co-Authored-By:` trailers from
143 commit messages. The rewrite changed the SHA of the **265 commits** in
`a3327863~1..main` (pre-rewrite naming), i.e. every commit from 2026-07-18 onward. The
392 commits before that range keep their original SHAs.

Nothing else changed. All 657 trees are byte-identical to their pre-rewrite counterparts,
author and committer names, emails and both timestamps are unchanged, and every rewritten
commit was re-signed with the same GPG key (`6A6A6C8413A83820D1F1E1BD346BCC41CF9D553C`).

Pre-rewrite tip: `8cc89a17786712c4c267aaf015bb8ed2ea87cc63`
Post-rewrite tip: `71028aca8c4b276f2b3053032e92e67b7e5af020`

## Why the evidence packets were not edited

All 43 files under `evidence/validation-rules/packets/v1/*/evidence-packet.json` record
`capture_provenance.tool_commit` (and an embedded copy inside `capture_tool_version`) of

```
13c7ee93d46f9b2e70ee24e8d4a2a1acd76fd419   ->   d4db979b944c0789ed1c64561a8c63359e1114fc
```

Those files are **deliberately left unedited**. Their bytes are covered by a three-layer
sha256 chain — per-packet `packet_digest_sha256`, per-packet `manifest_sha256` in
`evidence/validation-rules/packets/v1/packet-set.json`, and the aggregate
`packet_set_digest_sha256` — which CI re-verifies via `check-evidence-packet-set`. Editing
a single field would invalidate all three, and the codebase has no in-place fix-up path.
`tool_commit` is validated for format only (40 lowercase hex,
`crates/bir-rules-codegen/src/vault_acquisition.rs:910-921`), so the stale value does not
fail any build. Use the mapping above to resolve it.

The pre-rewrite history is retained locally under
`refs/backup/pre-trailer-strip-20260728`, so every old SHA in this table still resolves in
the maintainer's clone. It is not published: GitHub only accepts `refs/heads/*` and
`refs/tags/*`, and publishing it as either would defeat the purpose of the rewrite. A
fresh clone of the public repository therefore cannot resolve the left-hand column.

Two entries in `docs/validation-rules/worktree-cleanup-inventory-20260728.md` were also
left alone on purpose — `/private/tmp/ebir-native-evidence-879554d-w1I01R` and
`/private/tmp/ebir-final-macos-diag-923878c` are the literal names of directories that
existed on disk, not pointers into the object graph. Their embedded prefixes map to
`b1adf535bb67…` and `9bc840619824…` respectively.

## Mapping

| Pre-rewrite | Post-rewrite | Date | Subject |
| --- | --- | --- | --- |
| `a3327863d54091eecaeabd5e42969a7d1500e863` | `b503fb0182870088280743c51b57d51b35bc65a6` | 2026-07-18 | fix: restore distinct official form codes and gate specialized inference |
| `2bd3c8be4ad880d30333f798fd5f242e7de8fac6` | `dcdbd73fdf7940d696ab77b070e87107855f83ff` | 2026-07-18 | fix: heal partially upgraded per_year_forms schema on startup |
| `3d45476badcfa40f642889708bc53ab784dbff8f` | `a9150bef1a14dd1246131e47b258a206baecc52b` | 2026-07-18 | feat: resolve annual income-tax election eligibility per taxable year |
| `abd4a6cbfc06926c4f0e92b449765d9637201b9f` | `1460adefe8116ffc89ec7190ed28fafc98406f81` | 2026-07-18 | fix: suppress no-op change emissions in shared input components |
| `acbddefd0f9f0604317e34c65bf42931d52dc661` | `30251c11edc230743a182f0d4addbbaf8a51e1d4` | 2026-07-18 | fix: harden profile-manager save integrity and COR/Forms/elections workflow |
| `46629aacbdbd97d6fbd75b15eb9d576da1dc6f79` | `b4cb8b922a8c824edf05a2464b94ebb4ae3fd4be` | 2026-07-18 | fix: enforce a single annual income tax return per taxable year |
| `af5a0f46c34d9915ae990cf72b8405058b561eab` | `affbe7375ada72fc1a897297534c957b97a2543b` | 2026-07-18 | feat: per-profile Google Calendar form selection |
| `c95e5c5f3df2709e16068c31a1461a65f9b4ecf9` | `cb862d351cbbb3a9585a7af390348856d4400a97` | 2026-07-18 | fix: theme-aware contrast for warning/status colors in both light and dark |
| `b23a1b19e122303f5a30da4ab4bc294346bf6945` | `af40a80e3a7ed6b2643cc98daeb8e20ce52e068a` | 2026-07-18 | feat: add/edit Tax Calendar Explorer overrides via a modal |
| `c39e1c89130a7c6fbf062d0fb96bddda92a90924` | `3336c8fb084bf69dbd997f559733ee3dea98af8e` | 2026-07-18 | fix: dismiss the MultiSelect dropdown on outside click |
| `c43e5ebd473c5b4e9bdbfbbd60d4fe5ada422d78` | `778154a67279325860a07cf7a1ef3b6cfec58b59` | 2026-07-18 | fix: make the global toggle hotkey record, register, and toggle correctly |
| `4de541e4a33065c38f91249fdfc67b44d71ffc5f` | `1a53b005152d15c874dda467e1e52464fd6eacc7` | 2026-07-18 | fix: profile-manager ITR guards, calendar form UI, and modal/theme fixes |
| `bce6dd975cf42a7c31dca681310a7f4cc5c6eecc` | `f8aec032f9d8518b8ec729e1d7011415c6116fc1` | 2026-07-18 | Serialize profile save requests |
| `5e860541b45098aff88658b0c911caaa34007c84` | `2e88bc3b3466ef3e69f838edf7cb3fbca619fa20` | 2026-07-18 | feat: add macOS native evidence exercise |
| `39c2765ce3b36eb7cdb3a06f9bc4d5b88b0937e2` | `80bddefc86650c97bd7e3744cf664fe98ff75078` | 2026-07-18 | style: preserve profile manager formatting |
| `deacdc52eaffb3ae0a7bcfef09e719295e9cced2` | `77bbf121e71405f2b11c3acf84d7a36b71a312ee` | 2026-07-18 | fix: preserve official 2551Q item 17 geometry |
| `e930d914edafdc7506865f1608e01f544340c53b` | `1644927e59ec820690bd509c26e23cbffde03b72` | 2026-07-18 | feat: export live database safely for diagnostics |
| `76bb61b537960adb3b350ce3fd4df94d6aac581e` | `40744179d65f95c7fa0dddf8805eda944ce093a3` | 2026-07-18 | fix: fail closed on unsupported tax form inference |
| `98fd6626c43b7e41a14cee3808f56724b19b6ace` | `12d4c3d493d5c1642afa3df8d6dbdf373c59c99d` | 2026-07-18 | fix: deactivate stale migrated Forms Set entries |
| `a395de0b3975430bd680657f160b2c7b11ad8a3b` | `423a1fd2cd2c43b5ecc15e3c1ef2af753bcb69f6` | 2026-07-18 | fix: persist pending income tax elections |
| `8a3813b8b228a2c7ff42322c37cd4952c6ea4f4b` | `11c987161ebb20139ccc8873d1e4fad79086aa4e` | 2026-07-18 | style: reduce 2551Q raster divergence |
| `0acf86f016a95debd72840d8517aa0ec6b931949` | `0a1b8a841e193446ae2cbea0ecd275bb60cc94c4` | 2026-07-18 | feat: add packaged macOS output diagnostics |
| `dda47ade2c66d3266fff782f82fd717f53210dc9` | `96c62b14f6cbc28d9732247ced2c8752d711ca71` | 2026-07-18 | test: preserve reviewed 1601C form copy |
| `d9dd01fe6de8d05d72937d1c5055b975c107e1d9` | `7dbcd6c381cdb42ae174878e1197dcc2523e5b6c` | 2026-07-18 | fix: restore 2551Q edge fidelity |
| `9b7b8512bafcb0ee4294f7f0f5c1165ec8c9b3f4` | `b57d64c12feb882ee20419c809449fb07097e6c6` | 2026-07-18 | test: add layered 2551Q fidelity evidence |
| `5d713ab264fefeab9fd0dffb37c78c8f8f214bc0` | `844b3f76a8546990f67ae96d7fdcff38b427a6cd` | 2026-07-18 | fix: make macOS evidence window automatable |
| `997d39f4777966c363ddd39635c7bba4fd59233d` | `d5d3bbc327913aa1e146d6d4530f24eb34086051` | 2026-07-18 | test: strengthen 0619E evidence gates |
| `886826d68c828174b004306ccde23e3942dd9561` | `5de7189583e714b1d1da8de5fa77937b3f669d20` | 2026-07-18 | test: lock 0619F source and form copy |
| `fa1fc2e481ba591f2cdbd5a720f22a9882566d7d` | `487d253cbeaf28c71a1d4f6053969e76bb6bd246` | 2026-07-18 | test: normalize 0619E reviewed copy checks |
| `4221411ff5e72145b16417242e391b55d16df787` | `a706a33722b484f0044d5597b2330277397fc246` | 2026-07-18 | docs: record honest HTML parity blockers |
| `87bcda8e85e9a6be32fc4b04c65ed1ad1c46cbbf` | `adef1acb4f7c1bd7d5b9aeae39d304e1b6d4b4a6` | 2026-07-18 | test: strengthen 0605 source and field evidence |
| `9e26ea7fc6e5bdaa20e003e1e2e56e8b6eafffbd` | `643a221b8647816a04a908820da9003d2e90c6c2` | 2026-07-18 | test: certify 1701Q source and form copy |
| `49b2ec85bc484581d73d71b47a4ae1fbc2ff7e00` | `28eea67bca00f37893e4a8743837d499a86c4326` | 2026-07-18 | test: close 0605 visual evidence gaps |
| `8108ab290ff2c8c442f8bd1d83ff4f8eeea422e5` | `9259d7ae01a6424a06ab9422334c198584b7de90` | 2026-07-18 | docs: correct offline renderer size evidence |
| `3f994910a4a861f2f460a5aca13d5dabe93f348a` | `a2f5832166935a66ff4a3e26e3ab5597e449db00` | 2026-07-18 | docs: record 0605 and 1701Q raw parity |
| `92149a73d6247309b3e74a7d75fb11649f93ab09` | `1cc13a05b7e2c21f24a591c89d04edcd284cf7c4` | 2026-07-18 | fix: keep macOS evidence toolbar on screen |
| `168b03c480efc1f265deadcc806c6d6dcba0e2e4` | `9e21d46ced222046244211abe36a820171621d64` | 2026-07-18 | fix: clarify HTML preview output controls |
| `2028b16e419696a6122c4cba7ffc23746154ca63` | `a8d2a18b9b54859d42478c6a94196dd9cda655c3` | 2026-07-18 | Certify 2550Q HTML source and overflow evidence |
| `865f75ad0f112cf49aefbceba5dd125de47c6434` | `54ee802aca3e9d482f3d2d9ad4627c105f7833f2` | 2026-07-18 | fix: bind macOS evidence automation to exact PID |
| `ae7c91b0292cf6bef89f80ca33dc5b333035db2d` | `1af1064c56ed7561ed6ec22870a791a6fa86a399` | 2026-07-18 | test: certify 1701 source and renderer evidence |
| `53960abe3a046b3131e8efaa2e331687b4669bc1` | `0eaa2c54b0bc14355a8ad1a8f101c49366ed671f` | 2026-07-18 | test: automate macOS native output evidence |
| `84cf6d342eaeb03955a07c414edd990b39805159` | `4c6ed59f4bc7ee9e11c29d7b277a009763ccc8e3` | 2026-07-18 | docs: clarify macOS evidence automation |
| `3755b5bbb545079a446b5bd2eef29da2bd7eea7c` | `cb9c48bcdfb55d12daa7286999947a859025eea4` | 2026-07-18 | test: strengthen 1702RT source and parity evidence |
| `879554d6e57c089e3dc3d36cb78b7769929734cf` | `b1adf535bb67183e7e7a93ce92c09fab7373aaf5` | 2026-07-18 | Certify 1702MX source and renderer boundaries |
| `6848f9fc8e2fc592a48fae82ecbf6bb1d5e00493` | `87f8ee56db615112b252295a8b502ae076d0383f` | 2026-07-18 | test: strengthen 1601C source and renderer evidence |
| `1d8f941afb6aea5013400b1412732bf9e280d56b` | `ce1781b345414022b36cdfbb978ca00e8be16cd8` | 2026-07-18 | test: compare 2551Q blank reference at official item 17 geometry |
| `981ff8c593c034671a6b80981fc35903cd296d08` | `9fffa368b8262994c6ec756df8808b8a982baed7` | 2026-07-18 | test: enforce official 1701Q payment capacities |
| `4d872d3e778c5298ab3ccb3db8f4d0a3bfedf574` | `fcd5eaeb789625b5a0ebcc16f79bf5e666d2e1e1` | 2026-07-18 | test: strengthen 0619E source and renderer evidence |
| `ea0b3ca7880226b872d93c8f33e5427a311e870c` | `734b7bb784e49cd5898384cab7b8f8d09f531836` | 2026-07-18 | test: pin 0619F XML source variants |
| `d856621630eb34cd00f7cab187e03c90444dcabb` | `5ff35d8e12270ffefd44d8ddbba10d804cb3c512` | 2026-07-18 | ci: add HTML candidate certification bootstrap |
| `bf17bb929b5ea9d480a8e404496ab4b2f16b0677` | `b9ed891ffc82aa26b7999a51b9c419cc3cb96677` | 2026-07-18 | test: satisfy strict Rust lint gates |
| `4ae283b416674385195e47197cd7a97d119ac310` | `f016b0331f2d4527521f7daed5336e266294b8aa` | 2026-07-18 | style: normalize Rust workspace formatting |
| `64f5da724b5b68fe9e4d1f69d906a69bb1f914b5` | `72286796e7f6ce8226f24c3bded78f09993539af` | 2026-07-18 | refactor: satisfy desktop lint gates |
| `79b470a101034cd32e3b0e4a13c563b253db581e` | `d2fda1b75206ebc94125093516e73838eb82102d` | 2026-07-18 | test: lock 0605 official field semantics |
| `1d91a55209ef8d402cff06fcc8fac0a7c2b62c8a` | `bf49f7e2ddcf440682bffbe8cbe421708135519d` | 2026-07-18 | feat: add macOS candidate certification foundation |
| `f4dd762f25a67af81298ecacf69711b8634fc65d` | `8c12a2d176a7e540010f5fe631bcbb16ac5830aa` | 2026-07-18 | feat: add Windows candidate certification foundation |
| `8e4f80a111f86de592c15bf0a3d35480645a6f84` | `b831c5549f05ca4769fede85ee2d2a2be4203f28` | 2026-07-18 | fix: restore 2551Q header bottom rule |
| `af616fcb4d2e1953263358cf5ca52ce0791e8e3d` | `0d393b9b33ff8f8f6274027334dff897b864b734` | 2026-07-18 | test: add Linux candidate certification foundation |
| `7071915880e4bcf0688b03b83c26b89f49c114e2` | `addac2803ec524bf7a5a985a87b5347c1c606de8` | 2026-07-18 | fix: stabilize 2551Q page two title preflight |
| `ca5aa9f04634d18e76d570cd084b75c2f6a30e7b` | `b9187a25285b0f2b3209e6258d5b9ac50984ba8d` | 2026-07-19 | fix: calibrate 0619 remittance form details |
| `a9c1949769b839eb2a94c5d9a4586f5bff94585b` | `45015510e4713ad45ad888399da036261f51e58a` | 2026-07-19 | fix: calibrate 0605 ATC reference table |
| `bf2d27890fe116940c56b9901e37e447fad190bf` | `61a7e3f24d09b4a0388bdaa0d1a60d51d0adc0cd` | 2026-07-19 | fix: calibrate 2550Q government header |
| `41771aa168e5b0d244b2f4fb558bae44fd6c087d` | `1808b2667ae8737634c736135b93956031abaaaf` | 2026-07-19 | style: improve 1601C HTML print parity |
| `374ce184590d383e4a23350ab3c494e3a51df8fe` | `22111f17be0a77a4cad48621be6e944d99530db6` | 2026-07-19 | fix: align 1702RT continuation partitions |
| `b8ce163c752596040e8e5b30c95e68a1bcfb4e46` | `f10ccc9ff6cded316711d25ff15c31fe81ab51d5` | 2026-07-19 | fix: calibrate 1701 taxable-year guide |
| `c2ecbfbedc62d9f9a21d198f77e7aecd0556e935` | `17ae3737eb0633f2828eed49c7a69edbac4bdfec` | 2026-07-19 | style: improve 1701Q HTML print hierarchy |
| `6b21c7720de399ec7343156bdc4cfab71351b86e` | `ec140fc8786c10fc838be08b3365d023fe977ccb` | 2026-07-19 | fix: restore 1702MX Schedule 1 geometry |
| `aab288417788038a6466ac65279844dbb3da1de5` | `ae32bce885d6c4cd9889ee4ae6ca16928d298475` | 2026-07-19 | feat: enable immutable 1601C submission queue |
| `e9de8acf99274fbd9e39d086d4fdf489a05fda4a` | `4768f46fa4786e100ca41dfefd4c42062f95dd9f` | 2026-07-19 | Improve 1701 page two parity |
| `9d7eca79c359486db4595a62fccf6b75156ec49b` | `0ae3b8c9135645bb23efdcc9c01a9f0c8088013b` | 2026-07-19 | style: smooth deterministic form typography |
| `b41ebd5fa34d7b55bc50f2ba521bf883c4b13a00` | `c660826d7d8e4f4504e8e64969b0b6aa57815b21` | 2026-07-19 | fix: align 1702MX page 4 schedules |
| `e7309a525ea696e9ed7dd77761f7f969f1650660` | `8eb4af99aef21523e7109e36f49229573a99b69e` | 2026-07-19 | Improve 1701 page one and three parity |
| `5ee1031cce10198172b159c4ca1e81ebcdaa9bda` | `243bfac85e93c12cb85743386b965f3425f7cad6` | 2026-07-19 | fix: align 1702MX pages 1 through 3 |
| `a63dfc4716409c00d70be134cee218c1b11c6025` | `f936d64247a0101475c6b0b34d104ebfee985794` | 2026-07-19 | fix: isolate macOS evidence startup logging |
| `509c703cf923f115d205e44a327f32ffb6f01951` | `ac1344cf97611ee21267cd71fbcb869a74b60efe` | 2026-07-19 | fix: calibrate 2551Q official typography |
| `963347316f68a82f0d9ee652aac46a37525be720` | `e0bc26eabba9133dcc38900b5dd3c2d0388c18e1` | 2026-07-19 | fix: refine 0619 family print typography |
| `d84b90b5dc11079f030526c07be26e564bed861d` | `c1da3f594ccf5867d6bed4f309b0a6de1120af5f` | 2026-07-19 | fix: refine 0605 official visual calibration |
| `cd774ca9678b01b31e14342b5c9d213260a2dbe5` | `0a72cfc88448343f67835b483d23786c0e7fc645` | 2026-07-19 | style: refine 1601C source parity |
| `c0ee78a8c83ff1d4f0cb44bb27683b3556b2a8e2` | `1cfda4c8ef41bbd5e4e6ad73c1734c774789807b` | 2026-07-19 | fix: refine 2550Q official visual calibration |
| `738a307bbcfe99b4c2869d38680cd56a3be5177e` | `0d6f14b8baf5c37f47eacf14ffad48394bd2f544` | 2026-07-19 | style: refine 1702RT source parity |
| `a5f5c18f8af5435447efc8dd409ce24ea6fcf092` | `e25b20cd2f82ffc498acfcb20bf00e991b7065be` | 2026-07-19 | feat: certify 1701Q editable XML contract |
| `f3823134263fec5005f6bf9ebaab1f49102dc5bb` | `5edf5f0ce44024a7f04c69162a0830d77922fd71` | 2026-07-19 | fix: calibrate 1701Q official form numbers |
| `daeb1d257df75ea963fe77f4df08a7d97e0d97de` | `2f98f38158e6b9d05a4efa8cc29da59a9361085f` | 2026-07-19 | test: remove redundant 1701Q capability assertions |
| `f0fd32924637fcb027f624f9585611971950c9d7` | `969aa4052f095bccfd9a17a22d81158d406bc95d` | 2026-07-19 | fix: refine 1701 official visual calibration |
| `d53cb8b431eb1ccf3f1602555d98208fec384038` | `832f97d5c778672a35dc4cdcec5d3eb3b20cb305` | 2026-07-19 | chore: keep 0619 submission fail closed |
| `923878cc7247f66eeb06330a84c781857b24b9cb` | `9bc84061982458d3e9f0f6d0bec9579096ffabcd` | 2026-07-19 | style: correct 1702MX plain amount fields |
| `42534671a0b22a3804ae6dd414b7f32f76ed6efd` | `1d962c3668ccf578a60ef16191f15f8e2b67dd17` | 2026-07-19 | Fix macOS native print dialog diagnostics |
| `ca2dc0caad98999ae6829d831a96b1914c44d405` | `7feb0952cb258b83326f0af685a8366d54ddd83b` | 2026-07-19 | style: refine 1601C boxed-region parity |
| `505d1e5655e5d6b4c8125845e7f4db4c948c19e3` | `0f95faffb2483ed76aef6394ad864724138f4971` | 2026-07-19 | Calibrate 0619E semantic typography |
| `735cd63c1ff820cfc89cd7ef3e0e0cc7f43a5998` | `ffa0145225bcf05bf2418bbbadc744e6acae68f5` | 2026-07-19 | Calibrate 0619F text baselines |
| `754b32f2ceb9fb06c755a9860cef9b11a1b92f24` | `ebfefe7319c51b934f57e2d063de5adb73cb18a0` | 2026-07-19 | fix: correct 1702RT schedule input partitions |
| `84657819ce0b816e1c38acc0dcfca0a9c81b39c0` | `590ffc738c6e58f0612fb42d10d89874e9b81c5f` | 2026-07-19 | Audit extracted local Windows installer |
| `3b3f490db0b60a008f5c06872e6c5d604d6cf89c` | `2b39fe9f831efd2c700458361cecf365a2050c8d` | 2026-07-19 | Clarify queue-authorized form transport IDs |
| `b8d9c9480a74cb7855dd596a148ed076cccd407a` | `78c0492ac9c1bdf1600d29a587e2f8f7b4efb76f` | 2026-07-19 | Validate HTML form conversion skill |
| `10d739ea27f5b39a11967becffb60d330534de2d` | `111cce2f72e5e1fd7bdd38a4c74afb1880069d0f` | 2026-07-19 | Model Linux native output observations |
| `c07dd78884cebd452e261ca72c8f093e2c830d76` | `19ec1625aa64d3ad3fecc521c5a4c85a9c2e9e28` | 2026-07-19 | Record HTML-only package size diagnostic |
| `6a845bcd86c75da19e5dc15fb872af8b97d28f6a` | `7b0dbee66476e052c974825cb7bef4ad11a8322e` | 2026-07-19 | Restore income-tax declaration fills |
| `2e61b4c94886205be7b7ac15d25d5e9e878438bc` | `964aeb8cc4ebeff256bd38d6214c577d2061fbbc` | 2026-07-19 | Restore 1702MX unavailable schedule cells |
| `faecf6f332d15024b02bec29cc0f3b4819302b51` | `09df4ed88a65800706f5fc9e9acd3e3fbc588e3e` | 2026-07-19 | Restore 2550Q official gray bands |
| `60e3dae5ffe4a8007c59f387fa3a0fc04d0078d6` | `a19adedf332a5776e190091bcc580c74625682a6` | 2026-07-19 | Use provider paper geometry on Linux |
| `2b77839fac96e64713f3515cbf6ecaf9aa0c629e` | `a9ac3b98d5c95ea217261a4b1090cade5b310d77` | 2026-07-19 | Calibrate 0619F Part II row heights |
| `47ddaa52723ccab2aff9a6ea8fc6d27c529e2d94` | `03a1064735f5ac243e798ef0c5072c21213b37d1` | 2026-07-19 | Exclude 0605 fixture amounts from blank comparison |
| `1bde72bfcabd08a6ae3d6de39bb99668fe98b4d3` | `37e1294d15d1c239bda630a141c7e1faf4c589d3` | 2026-07-19 | Synchronize HTML form readiness evidence |
| `cb6c4c0e1c27b32f9586085c1a4c3c9050b10dbd` | `95c4a290f0632766210c18b849088ff2f8ce3626` | 2026-07-19 | Preserve source geometry in appended PDF pages |
| `8dc439f46dcd865fa30bc6447490cb3342138b13` | `57ac2577ef27bf541178bf2b41c236523f43e019` | 2026-07-19 | Fail closed for uncertified 0605 lifecycle |
| `f1941ec676e180af2ddd80aa6cc820667039918e` | `1bebba601125a07bcde51fa909ea77b0a1916835` | 2026-07-19 | Calibrate 2550Q page one typography |
| `518e6a78de33150a930bf1e9a149b22f5ebb852b` | `9e926a0b1aafd8235cc80e683af72fdaebeeeb9b` | 2026-07-19 | Calibrate 1701Q source typography |
| `af6d08fed9ec216bc15950cb09a1c71c6d919d5b` | `097157518ab17edee90ba331901ad84f78a5e4b3` | 2026-07-19 | Restore 1702RT schedule guidance |
| `3ecdfc2e085e446abd4767d03c201d0f51eb37b7` | `8e222b322ab9ace1f3de5f4119cd3a4928d168c9` | 2026-07-19 | Calibrate 1701 page one typography |
| `2a97327a39c0e13d372abe33a5d7851ca0d75a5f` | `fc0c961d11ce91fefabef188bf1829b6e1d1feaa` | 2026-07-19 | Restore 1702MX Item 5 boxes |
| `1216b55f6ef909ca2cd3d9236f89ab162aa82599` | `863ef128c28034e955720a5e48a521641327a548` | 2026-07-19 | Restore 0619E checkbox interiors |
| `c48e5fb1d97b8d78a54ce943561133ca78d2aa3f` | `b0e9ab1df9e4abc56107df8bb36d323d32dd690e` | 2026-07-19 | Require notarized macOS certification candidates |
| `049eb44527ae213767bf78d6c25df7702c786b7a` | `c31bafff97677a6504dd968e83360397b2985326` | 2026-07-19 | Refresh strict HTML parity diagnostics |
| `5a2b719813585a64b5bd139614487c9530f0ccb5` | `246aa278fec82d221ceed90073b7229e7df3ded8` | 2026-07-19 | Bind candidates to curated renderer revision |
| `f254cac802594a0a5bef50444ff961618b344eec` | `30686eccb526e83d79d0bbd9b9aba12dfb224e4b` | 2026-07-19 | Record remaining form parity metrics |
| `b7a5a77ebcc68b3b096bff0949e707d3cc56c51e` | `7e26258f8745411f2d138612e9e4d94e23ce45c3` | 2026-07-19 | Cover local-year reconciliation boundary |
| `ab7b81c9564bed033e9be0c78b60674fa389e230` | `908e18fd6d7682d46d51c7bf4f8dc90fbf21deac` | 2026-07-19 | Add non-promotional macOS output observations |
| `d08688b0928a8cd924249de52cfdaa63426335e2` | `b15e19fa66c389d1cf3d697d6f3b23b9f31cc05a` | 2026-07-19 | Harden 1601C queue boundaries |
| `f71c20ae6c35e1f1668464ae22375f3aa0989b23` | `37376376a1c36889cf5fd585f830d7ae55d215d8` | 2026-07-19 | Add external macOS candidate collector |
| `e25b17379f167bca61489260d773653f4ba9da85` | `5714443da1e276bef02a6b0e1b83b1dfe3f50fe3` | 2026-07-19 | Certify 1601C submission outcomes |
| `af866c380fd32c2e02fb3274b366d2eac9f2ce11` | `bb92a51eb121160a5e6d0a33e1def05f7e40e798` | 2026-07-19 | Add external Linux candidate collector |
| `fc3e16b91c89f5325922d79d1c5e32390594f40a` | `97dd451d0ed99fb48675096e6a34cf658aa48e94` | 2026-07-19 | Audit 0605 and 2550Q submission boundaries |
| `c6a59ab1caeae76ad1beff82d58694175ed3ae8d` | `0b0a679d0e85f34dfc915a46ca71bc8897c564ad` | 2026-07-19 | Add external Windows candidate collector |
| `3dd16ec1772e8d11299aeeac360f26727fbca744` | `4a08a37e5bf2327447fbe2ff47a6c027b06e5421` | 2026-07-19 | Fail closed when HTML output nonces exhaust |
| `eeb1c8d9f6f3147dd167a10e0dc07aa1025419f9` | `6ff3b7af101e6fa8afe0895674c07ca93dd80e8c` | 2026-07-19 | Calibrate 2551Q ATC reference typography |
| `11ca65008090c162a121e07aaf6b5a59273216ec` | `e58ef3150e858d49f1a753543aac4a76badefb86` | 2026-07-19 | Require hashed release evidence in conversion skill |
| `5fcd78e2f7489f29bcf9b374a38fd9eef9fead3f` | `c69980d941974f21185665b1442ae62d406f0532` | 2026-07-19 | Calibrate 0619E use-only header band |
| `b8156d2b8b312780130a66f7074fcadc403f871a` | `f7529de9a192fc2a808349ccf8a4ae82ae0940eb` | 2026-07-19 | fix: align 1601C guideline reference typography |
| `672e04e9a65518cd5b261270edc80c9301e48269` | `e440bf82eaaa163e70346671a674f7a3f60d6cd0` | 2026-07-19 | Calibrate 2551Q page-one payment geometry |
| `4a63ae037a22adb515e80cffa2fadb41034a8768` | `bebd59a6749daad3f35453a2eee6e484e38dc489` | 2026-07-19 | Correct form readiness evidence metadata |
| `54640fc58041e119f1f8b6d1abfe1d917d390910` | `6cdf3d7c3591d4ae519121bfd889e2e5850ddb63` | 2026-07-19 | Add same-rasterizer chromium reference pipeline for 2551Q |
| `d0a19c693734b7f963e67cfb1ebd94f66f672dae` | `77788876164f556a1a49e9ad1c56f52fe885ad63` | 2026-07-19 | Pin 2551Q chromium rasters in the reference manifest schema |
| `d44e6923b354e7ccaa26be369887af04b42ebe19` | `c46a0299bc7439bd8156b8233f22206d6d9309bd` | 2026-07-19 | Teach the migration audit the same-rasterizer visual schema |
| `0b266ee7b9593b5f5997754ac1fb327959c036ad` | `163b2c72db8298729c64ab4b0544d1c7d7f1bdf8` | 2026-07-19 | Gate 2551Q parity against the chromium raster with region-ranked diffs |
| `0c055a7b5a673eea5b383a13789aeeba595a10b9` | `affe33301cbee6b208295b38b735856f70c0ddcc` | 2026-07-19 | Promote the font-attribution sweep into a tracked diagnostic |
| `6c2c742f60199a90e758086ea84f959ffe81ccab` | `b14d2eb5c90b4cfd73d570c039fef987e8fd9d8c` | 2026-07-19 | Wire the visual gate and evidence rules into agent guidance |
| `a40d8a84b000655d147ef2307c269b2bb83ed277` | `27d3230d552f35e9812c3b9d9d483705568892dd` | 2026-07-19 | Add the visual evidence producer, runbook, and honest re-baseline |
| `dd21c0fdcf820a9b6d00b981fcfe8c5b6919846e` | `8b1202dcbbe63c7e8e126c04cf8a4d0ea60d9813` | 2026-07-20 | Correct the 2551Q residual attribution from glyph shape to geometry |
| `52efde094834a69c35531447bc9fbeaadd18991d` | `dbf881e2d30690843481f401c092dc7c01971a67` | 2026-07-20 | Record that the 1% gate is unreachable for 2551Q, and why |
| `a8f2e87a323dc82af6950a649ecf80ed98ee6485` | `2854966d2bdd5602c32646769b16a8f24242bf5f` | 2026-07-20 | Specify official-fidelity-v1 as a non-regression criterion, not parity |
| `8c36e3e223f98976f8e64cedadf526acfbb94cfa` | `81dc19fd0095a41f1701a2403117912d379872fd` | 2026-07-20 | Record the text-excluded structural decomposition for 2551Q |
| `3557409545ab5629c8ab36a135d5c6b356354922` | `2a87fff83380ec60b37cbb69ef1e2a3b3a503457` | 2026-07-20 | Adopt conversion strategy v2: structure-first, extraction-driven, 35-form scale |
| `c44e6b7c333a9e4a28ef8667071ab94397eafb49` | `454f32a1df06be0fed0c6bbf864524cc3d59a843` | 2026-07-20 | Implement the official-fidelity-v1 pixel components in TS and Python |
| `c450a71c8733ccb86f8365ef6c651a8612bcb9a7` | `dda76492ca700b85b9fe2e42a9071e4273508ada` | 2026-07-20 | Add the criterion's own regression test and correct its content-signal claim |
| `41d09c46af87d7b2a08d15bd333ed193f954bbde` | `30646068d9216e504c09b9eadf1efe9b7c41f482` | 2026-07-20 | Add encoded-artwork-integrity-v1 with both required bindings |
| `c1024dfd0aaf02a1d0565700a11867b3f68bf40c` | `bae12c8c19e8507da53f0370fcd287b1f77c842c` | 2026-07-20 | Add static-text-exhaustive-v1 and close two bypasses adversarial testing found |
| `a26e6f25c3c7e040bf264efbb79612d636febc12` | `4c145d92f24942109a9f181626af972219220b78` | 2026-07-20 | Emit official-fidelity-v1 as schema v2 and make overclaiming a hard error |
| `5bbd70d48228369a9e4c4be02ba6fe911a705fa7` | `b1a5eebfb8c1adebd4de6e7c0c7131a6469f9097` | 2026-07-20 | Add a structural-defect localizer that names offsets, not just totals |
| `13de19b27220e2c5a99a0c6bd65362999dbfe551` | `84ad329aaa6d6b5e62fc352c6235549f28b546e8` | 2026-07-20 | Fix 2551Q page-one structural displacement, verified against both rasterizers |
| `a38db3aa4c04d8c94e3e92190d63adb9417c2810` | `0ad688fb4fbedbc7b4801f13b16aa44b96c65139` | 2026-07-20 | Teach the localizer to tell weight defects from displacement |
| `89af41cd5f5b8076aaa7904017c085dc3c811b62` | `7f47e33981c57d3b1f6c31e4824ce047ef34178b` | 2026-07-20 | Correct Item 12A to its measured 16 character positions, not 26 |
| `65b8178f0f2dfb65b762014e19a2f53bb7c6a3a8` | `537c7a7a9daff4fe018df9d1a8d91c32536a8aad` | 2026-07-20 | Audit every comb capacity against the official form |
| `84ac99636b6d64ac5e9074388183a65624bd0a32` | `7e94adb5368bfd433b0131510c058b59426033e1` | 2026-07-20 | Add the geometry-contract extractor, with two contract bugs fixed first |
| `7338957c69596a17597261e774f460bdc3fa98c2` | `4991504ad9ae007949194cca6b5f79c61c99134d` | 2026-07-20 | Record the Milestone B structural results and their measurement lessons |
| `7f36751c2cbc0852adfb8288ef451ec2c3f15ed4` | `0e74526e13ab19e29c764eca7f342d61fdee4589` | 2026-07-20 | Re-derive the 2551Q Part II centavos column from the geometry contract |
| `d2f01513c5c4e8d24addba7c4302d02649283ee2` | `b8ea1d6aa94d1c0875c7b40b5b8e2d8b29602d3f` | 2026-07-20 | Retarget agent guidance from the unreachable gate to official-fidelity-v1 |
| `3ae29ac726dc7b6d0ebef73d0e7a47756cd118a8` | `2c33fa90ccae44b43477ad194a316ddb78bb6b8b` | 2026-07-20 | Wave 0: pin chromium references for the nine remaining forms |
| `3cb6ef1cc8e60a4425fa4319275db7df2bc3f8e9` | `e632a70d57cddd8a74454dc634945b78c68c6c20` | 2026-07-20 | Sweep comb capacity across all ten forms, classified by severity |
| `d5d0146dbfd86e77c5d65c80d1ce04a0ffc78e97` | `f3b20b9c1337ec96ef095e7deee8216683866c2d` | 2026-07-20 | Sweep structural defects across all ten forms, and fix the probe that hid them |
| `f15f652818b9280c8c4c7385582e4598cfc88a30` | `7fd6a952bc65820c019b17f17114cb86a30aa5ed` | 2026-07-20 | Calibrate 1601C, and find why the 2551Q weight fix actually worked |
| `e21081155a214097dc15587bc1217e51800a17c8` | `a67c2935af8a1fedfa5a81bc5f81931fd1a769fb` | 2026-07-20 | Point all nine remaining specs at their chromium references |
| `7106071c09e2bcc8db89e4c72754d97b892773d4` | `946817379261124a1f159a4ab623049ebb2248f7` | 2026-07-20 | Give 1702MX a PDF-derived static-text manifest and record what it found |
| `bad1afe88266977b898d0292bd73aaa43b569bb7` | `1c9e792222adcbf452cc7b2d2b62c043bd766326` | 2026-07-20 | Restore 1702MX's official instruction copy and ratchet its static text |
| `fab1f2b4c7a540b8d5ed7dc7f9fa5865a7dda7fe` | `607af56c28b287c18dd2e845f4acfad272410f4f` | 2026-07-20 | Sweep the three provisional fidelity constants and re-pin INK_THRESHOLD to 150 |
| `1771a09e3acb386239dacf44692c1bf70775beb4` | `b94be23c5df29c78aef701b7ea252b5a49d1e2aa` | 2026-07-20 | Restore the 2551Q page-1 footnote to the official 8.04pt |
| `b557f9a64a43c70dcb972f41808f6505169c33d8` | `1ff6d5c47a5f81982529ecf5c91828a1a65c17f4` | 2026-07-20 | Wire encoded-artwork-integrity-v1 into the 2551Q spec and pin its hashes |
| `67506e0332d4b85c9ae5f1890d6790b6fe34a211` | `5042c3eb2ce2c900ff3ce31ec39d458c57d40f33` | 2026-07-20 | Rekey the evidence producer from the 1% gate to official-fidelity-v1 |
| `dea32e1ff10a8f26a2df0829eb35d53c5a9b561a` | `93f0f9be81e76d1437247d4af7a162a4cf05357f` | 2026-07-20 | Lift renderEnvelope from nine specs into the shared render-utils helper |
| `b91b038454038f994f9f19307e78506acbbc7f38` | `c2e7ebfc44ffd09fb088d20b5cd5060ce7def324` | 2026-07-20 | Record that the sweep's weight counts are contaminated by grey guides |
| `d40a23515386bc9e51c8cf77fb608ce7ae54910e` | `2950e99bfacc63fec2a0938036586d3dbb36748b` | 2026-07-20 | Correct 23 charbox capacities across seven forms against the official PDFs |
| `dbee7636abc3c4538ab310d01f9c617a52045de6` | `4229db8fdfe8255e47d33e9a3b6fd10660a06987` | 2026-07-20 | Track the nine geometry contracts, and add a Wave 0 status report |
| `73bafd4e9fac147689be7102227c1666fe16ad85` | `d71f35104c75337cd2c240d125ba2ca131d96a03` | 2026-07-20 | Add section-crop review, region-table generation, and 2551Q's contract |
| `a6532143ae329c7f46f5c6d3f2b5d3e672b2b573` | `9b665052e0c5793097a12758ec6853f61dbf75f9` | 2026-07-20 | Apply the verified stroke-weight corrections to 0605, 0619E and 1601C |
| `ab42030660a4934a5ac162c1fc4e4996f216c7d3` | `9cfe06840f96752af0ff5a31bdc7e44d50c630f0` | 2026-07-20 | Make the Wave 0 weight criterion per-form instead of repo-wide |
| `4c754a38f22140f57258093eaedcc8c2201cd993` | `89061654d0bceb045daaa5c50b8d5e89e6954c60` | 2026-07-20 | Add /goal: a self-driving objective command, seeded with Wave 0 |
| `e993ea6f63760ba6fd1e8c33ec9949b0b6a1d4f2` | `ee0601a0bf49702d72c543dc31a0d70bb06ee52c` | 2026-07-20 | Render 1601C item 5 ATC as the pre-printed constant the official form shows |
| `13352e19ba30ef11a2e946c3ae9af3cbb59fd5ee` | `c9145cb37dda280887c410ba269285b9c6f6f7a9` | 2026-07-20 | Size 1601C header check boxes to the official 13.4 x 12.2pt |
| `b2e257357c0b341d976f8d3f2d89f6753c21e2de` | `238b4510d1d0e81b63b055aa846952d4b8ab7563` | 2026-07-20 | Generate candidate region tables for the nine converted forms |
| `910a169ae1f1ad78872631da4a5f9cf18a748a1d` | `fd370a66833e93a5f022ebf5ccceb07bfe32119f` | 2026-07-20 | Apply the verified stroke-weight corrections to 0619F |
| `67f705d227e9c2dea39628023d286f275f8159fa` | `93505df607b9e954c93c90312878ae023cfe740c` | 2026-07-20 | Apply the verified stroke-weight corrections to 2550Q |
| `c5d887aedcee7714419d50a039caabad68655935` | `226a2c0403bd76e77a80010bdc13bb660ff26826` | 2026-07-20 | Apply the verified stroke-weight corrections to 1701Q |
| `730339b6620f42e00df93fd221945ed67a7580eb` | `93dfd7dc48c6ad6c17f80cfcae37070dd20bc25e` | 2026-07-20 | Apply the verified stroke-weight corrections to 1701 |
| `a58cfe6c71e1541f3eea1cf6cda9878a92b48f7a` | `133c11d93fb4f61389e9a1505ddf635a06311400` | 2026-07-20 | Apply the verified stroke-weight corrections to 1702RT |
| `85525280012363a9683dd672fed8ac0023faf7d1` | `71d7de230f1f3243015b4e72b6195eceba3dae31` | 2026-07-21 | Apply the verified stroke-weight corrections to 1702MX |
| `a23cc53ba3e4f97142d41da1f73f0afae08b409f` | `0b9b4a5504e15c15aec524820821c91eed153483` | 2026-07-21 | Fix 0605 arrow glyphs, money decimal alignment, Manner rows and item 22B |
| `502778ea25d88290cf243d62bd68bcf0e81d2349` | `3817e4ff577059064838890e47446f3d47d30523` | 2026-07-21 | Move 0605 Part III item tags off the field borders and inset the centavos |
| `af88006b96b3adac08b81b5c98fa10e18070829d` | `d5602342318903327dce27dcc21e26de9e423de8` | 2026-07-21 | Add 0605 Part II box-left item numbers and align item-21 label |
| `d4a7ae16eff42bf5b73b2709882fb38b9fbd8eab` | `2abcaf4b74ea3ef0facf3181be69f3d9b58fa51e` | 2026-07-21 | 0605: render item 9 TIN as 14-cell character boxes; align TIN/RDO/BCS field padding |
| `73185e6d2190f052870eb937aa2884d8de8b5151` | `e535eb1c4a2c5ceadb28353355017b1e8cc313d3` | 2026-07-21 | 0605: align page-1 part headings, item 9-12 box tops and left-hand labels |
| `67ab721fc4c78fd2303537f0109d6909c2dc6bd5` | `765c1c33ce500eeb7391a2720875b370335c48a2` | 2026-07-21 | 0605 p2: indent ATC group headers and enlarge the tax-type grid |
| `f17cc5b16bb83e820a6dab46bef733749881d05c` | `38a217d3d721e8758f1b95181906d8aa003e139c` | 2026-07-21 | Fix 0605 item-5 "Attached" and item-20 heading overlaps |
| `d2aef417525d6dbc48f1872e0dd4a4dcafcc5730` | `8651f8bab71f090fb0b4f626b01da7bf8ba0620a` | 2026-07-21 | 0619F item 5: render tax type code full-width in its undivided box |
| `8361ca95a46c292f0cacea6ea88731319fdf3752` | `125a3e4e93da556e600bf178341f351989c417e3` | 2026-07-21 | 0619F: knock the item 3/4/11 check-box interiors out to white |
| `e16c49f004eaabc619158d16dc928e695a5e7b34` | `ba568d2ee81008a54de1e2fbb9b46e49e65e3013` | 2026-07-21 | Lock 0619F Part III payment charbox counts with four-state tests |
| `ec967e9ddb9db54bed7a689ec58dff27bf076329` | `6d6e3180d6f87544b2330c16137a68aca91f40c1` | 2026-07-21 | 1601C: knock the check-box interiors out to white |
| `cf63a71d82a437071f41fc3ac37822bc23023613` | `ab657372833dffafd01b6cbb83de012843a4e476` | 2026-07-21 | Fix 1601C page-2 Total Adjustment amount comb collapsing to flex |
| `aa97bba8597948eeba660811fcd079e4d0cf43f2` | `554be59fe05bea79b18b2ce63bf6a46ac0b4902f` | 2026-07-21 | 1601C: widen item 20/29 specify boxes to their official knockout extents |
| `e3d0ebda0148445ea8a7ffc1fc08fd6cee8d69da` | `6b87690057c8edde2bff74899c1e77fe9f0be40b` | 2026-07-21 | Add the signature line + grey caption strip, and two-line the right signature block on 1601C page 1 |
| `1ce742d79c8480ea188b091bb99610e6a7342399` | `d9eeebe0a54cd87aa95b79e739807d7b44b98212` | 2026-07-21 | Make 2551Q item 24 overpayment check boxes white |
| `023aaa6004a562ca0160c76bedab88d1753e0c97` | `8e22a1d4552a037a7f071396a38c28723fb68222` | 2026-07-21 | 0619F: centre Item 5 tax-type-code glyphs per half-cell, not edge-justified |
| `f1bca1be2c398275b3ef84c83cae6c8db7590ca9` | `d23cc150163f4b27e27504b041337b2e82932d46` | 2026-07-21 | Zero-pad 1601C Item 4 Number of Sheet/s Attached to its two cells |
| `b28f6fc1df25bd21a8ca68ae0c17fddf56fb3a3b` | `0e06648eed79b2748a2b8aad7fd935146304cf3e` | 2026-07-21 | Fill 2550Q Part I TIN row grey and fix Part III amount separator |
| `b5a4b5eba6281350c9486ec4c0479f13594a8f28` | `ddbd41b4a8bf9494ea7b5ff734f715e41b3096fa` | 2026-07-21 | 1601C: put page-2 Schedule I column reference numbers on their own line |
| `c934f0f7a76680e908f26a966d798c713a0d6d3a` | `aeda93bd0cf7211253eedd27e59e6161007e09cf` | 2026-07-21 | Record the owner decision to use uniform border thickness |
| `101ad313024a13c67ece4d95014844472f809b6a` | `e013abfb86c5c178391db2e562021194e60d6fd0` | 2026-07-21 | Close Part III's four reviewer findings on 2550Q (April 2024) |
| `e77410d3e6da7b8efa1b5f28f12b1d9ed9c4131b` | `c34c53dca03a327a38200b470203faa67fbabcbd` | 2026-07-21 | Add custom_form_styling.md: recurring conversion patterns for new forms |
| `a353fcbad39cd0ce92ff875831f97e4b6149af88` | `9a39a3cf549dd3ecba4fc5b917d55c5827012425` | 2026-07-21 | 2551Q: uniform 1px borders + drop doubled right-most comb dividers (SAMPLE) |
| `6076a98e39c6aabf4aaa1e44d7b183de1b80fa56` | `61799a88da53d0b4dc0b893b77027fc4f449f7e4` | 2026-07-21 | 0605: uniform 1px borders per border-thickness-decision.md |
| `db938aa56f7569ff8fc6cbfad9b969c2de294dc0` | `d17bef02a041ef76539ca3bb209d4a10a1c5706c` | 2026-07-21 | 0619E: uniform 1px borders + drop doubled right-most comb dividers |
| `0fb680b4daaa4d1f467205b5f9361212b2fca942` | `8eb81f5469b6f75ef7023de470f49393b6f287e4` | 2026-07-21 | Correct the comb last-divider rule: always remove it, no separator exception |
| `612926b196be82bb0a9cc5401042b38eddb0f0fe` | `82214dc721af63d4f64da0790b9dab32d08fbda4` | 2026-07-21 | 0605: add missing Part III row-23 amount marker; verify borders/dividers/seams uniform |
| `9c2f23fc981b6ee3eb4d496002a6adb21335e63a` | `edefa032dc928693a2e1188b35f91bceb0a3aad0` | 2026-07-21 | 0619E: corrected comb last-divider rule + money decimal-separator seam |
| `20cb83c93918e5e67c5828e983adf68caa415906` | `79afa199c384c217274e79276bcb1fa600b22d93` | 2026-07-21 | 0619F: uniform 1px borders, collapse thousands separators, verify dividers/seams |
| `a1dae96a70f28b5c2218bffbd01ceea7abdde149` | `33d33c733de59c045ed6aec74bf6b2da27218eff` | 2026-07-21 | 1601C: uniform 1px borders, drop doubled last-cell comb dividers, single-owned seams |
| `6c4fbd4427e34de3b9b426fe4d389f2eb8516818` | `4e6275f4564067b1998d38c0e6653355eae5f09d` | 2026-07-21 | 2551Q: drop comb last-cell dividers against the decimal separators |
| `f42a1c6dbd2c74038bc713af895abee791ec0db6` | `782550904db96d24a142fe8321b12fc9616ee13e` | 2026-07-21 | 2550Q: uniform 1px borders + drop doubled comb dividers + de-dup seams |
| `6fb74a75d4241805d74f67d3250f6ca1439e1bbd` | `64b8b1d836a8603059663a3abd8cbf5ca22f73da` | 2026-07-21 | 1701Q: uniform 1px borders + drop doubled right-most comb dividers |
| `98b342ed071b310c9c4cbcb8b1d9c0016f0cb52a` | `b142b1bb9ec311e8a892ff67652f4cc1a6e2bd65` | 2026-07-21 | 1701: uniform 1px borders, comb last-tick removal, and doubled-seam de-dup |
| `70a7e8bf62e588e95316a80e126306db107e081c` | `43e2cd0d9124c41879cf95e939d8e96b49ec882c` | 2026-07-21 | 1702RT: uniform 1px borders + drop doubled comb dividers and seams |
| `e7bf4536bedc7dc69a345db9927f3eebd99d42dd` | `0a4749727ecbd08870a0a9686e94c8ad9857bbd2` | 2026-07-21 | 1702MX: uniform 1px borders, drop right-most comb dividers, de-double seams |
| `1993856190ace7efe06f04691ce124b363bf1167` | `8f95fcc5722317ec7137cb2e831968ed7a5a47db` | 2026-07-21 | 0605: inset item-2 year and item-15 address fields off their dividers |
| `054f669f6a32198634d12f9bf362822d53a19596` | `feadd726c4630c033c62dc06f3fbaee42ff02301` | 2026-07-21 | Close 1702MX Schedule 10 row 10 box bottom (page 4) |
| `3a2f08d3574c3eee5b52b83b99efc0e5906d1a90` | `f4e3bf39270d5da68c2d1b99fbcdae6b86ff6810` | 2026-07-22 | 1601C: restore full-height guides on the item-9 address second line |
| `51573f2c2746b5244eba7fb5134bc3e43c8cd8d5` | `5492a7910b64d61f9e288a25bdab0b673b3972ce` | 2026-07-22 | 2550Q: knock the Schedule 3/4 total value cells out to white |
| `ce91f850700327880c648205e34849eb7bed44af` | `e5dd9991750e5bae1dc8abaac72610dad738c34b` | 2026-07-22 | Restore the 1702RT date/year comb guides and Part III date proportions |
| `66a17caf8cebe599adaea6c1e05ccfe892921ded` | `73b18d6de8a4ce7363794c179cb24a0154df9a69` | 2026-07-22 | 0619F: close the header-options band with a solid rule, no white gap |
| `a337b85f88d5db2f949e634b2f6b70c8a98152a3` | `490e5a5d6bff529f873892e1c8c0b22ce50b5e1f` | 2026-07-22 | Stop Part III's grey leaking against the 0605 footnote rule |
| `2e884919762daecb00e7eae77c6903cb4e20111f` | `1f7d6042632902d42274684c253f6f81c50974ab` | 2026-07-26 | Record the validation-rules evidence corpus and v2 candidate |
| `941407b3ca6a95e9ab5ee256c60524a45b0cc84d` | `5edef1979dc7ac91d1ed61d932896c3c14bbd1b5` | 2026-07-26 | Consolidate the live rules tooling into bir-rules-codegen |
| `288430666e856c62d4db849d8c22e2fc9d44fe5a` | `d5331418abae7c2b00d920eb40630512a3ca0331` | 2026-07-26 | Normalize line endings, classify all 160 occurrences, roll the digest |
| `ca64c6a3ecad9373554dfe09be739d7867375c03` | `a83f9234d7f9cf2b2dd294d67c81c0a6e5f59425` | 2026-07-26 | Add the GPUI validation summary view-model |
| `9e76f1d996f171dcb51a71a9d1fde4a17219c6b9` | `48a637ef6f1cbb3f7ff2bf7ec112029c7168b0dc` | 2026-07-26 | Give the validation-rules objective durable instruction coverage |
| `5db89ffa4115b7da2996cfec2117ebdf65da016b` | `207bf9e510614841fb1f965efbca87d5919d8384` | 2026-07-26 | Invalidate an accepted workflow transition when raw input changes |
| `8f2a6fe553921bac15152dcec7613756d20b0f9f` | `97c50f8e7bc849c9f4c065d6fcbbe7fbe8edca14` | 2026-07-26 | Report how much of the extracted corpus is actually executable |
| `ebcf2960394ce096bba06fe15112ab7ecae5500a` | `bc583aa5a0a5f30f4b4e108ba46f1999bcce8df3` | 2026-07-26 | Record why the GPUI validation seam cannot close |
| `9a9448d8a7671e48c35318e07b4cd107fdbe326b` | `e5f2ed548117deda3db09f4938660354e07a8d32` | 2026-07-26 | Let a complete draft reach the evaluator |
| `da2e5468588edf00c1535209cc988d6a619a9ea2` | `8b2cc4a607da7ab8ce7d40205850abe3fb879955` | 2026-07-26 | Update the action plan with completed work and the promotion path |
| `b13d7f31d592ffb7688b7c17fd35146b14cf3c5d` | `285f534ebbb78cabb6ea7eb56dd17ecfd7b247b4` | 2026-07-26 | Give shadow evaluation something to compare |
| `91c820e95f072a8e54b58a8e1a09391b93410e61` | `8410bb0d36fa554a4f7f0c31298adfd3e882f00a` | 2026-07-26 | Fix a duplicated criterion list introduced in the previous commit |
| `8827205ed76e4981159dae100be48f5326e8e857` | `02cca24d0c9d3e3f2ecf46fc4a16f51ea784eaf1` | 2026-07-26 | Regenerate the manifest after the status-criteria change |
| `f486e9f8104cccc8c0755dc7013f789f1eaeb23a` | `0e7ba52aaaa86e9b75fa21e10eee140214d8fe5c` | 2026-07-26 | Update goal progress: shadow dimensions done, two criteria open |
| `ed30a47930043a1508b527276026d6f2b5fc1f43` | `7f7cfd1c36998073e16188dbc5c85626a8c90cc6` | 2026-07-26 | Give the projector a staging root and refuse to overwrite |
| `2cd444286b3df2c5349be0048029dd5512b1dc0a` | `71696b18392b40c45c0a23b37413745ff8c447a6` | 2026-07-26 | Record the parity policy as a decision, not a task |
| `de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5` | `0cde6a1aa57aa18548a1a07e5a5e21c52cb60758` | 2026-07-26 | Close the loose ends: classification signal, stale validators, ignores |
| `13c7ee93d46f9b2e70ee24e8d4a2a1acd76fd419` | `d4db979b944c0789ed1c64561a8c63359e1114fc` | 2026-07-26 | feat(validation): add portable library evidence factory |
| `e772d390fd6bfacb983157cdb936517e400f73ed` | `0b5db4073bdbff7813d2079d2df6fefa9ab5d999` | 2026-07-27 | chore: ignore local validation artifacts |
| `f2e78ef52ce1bc9ff9dabecea97590c09ce84c46` | `f5f782b25a40266e190072aae22fc3f9d56bc5b0` | 2026-07-27 | feat(validation): build portable candidate library foundation |
| `2dd3810cc63c4df97974b120f9da6b71f2c99fab` | `9e139d03ce08fe1278663e81f2ac4d3048e87c6f` | 2026-07-27 | docs(validation): record macOS continuation checkpoint |
| `f5300bcbe2ae3347a206389d5257d0be6c678079` | `0e496488f250cea82751a23dc1f2d1b75594c75a` | 2026-07-27 | fix(ci): make the workspace clippy-, fmt- and test-clean on all three targets |
| `c0a6b8818b04814f6963fbb9fcf8f8f6799f3302` | `9edef6cae119bec6cdb2b0b6bbe6d0a119299d96` | 2026-07-27 | fix(ci): repin the 2550Q visual fixture digest after contract regeneration |
| `e2ec8965955a37809863b9c46c8c2ae6c598b87f` | `147ed4c81fbb6b7499723c4813eb4fecaa5b3299` | 2026-07-27 | ci: report the strict complete-page parity gate instead of blocking on it |
| `91f356528f01970ff7f09ff7784a7bd4bbfcd630` | `dd7a0136e4c81e8ab65f9d147292907fcec4cc9d` | 2026-07-27 | fix(2550q): scope the raw-authority requirement to checked XML, not preview |
| `1191a10014d9ffb34d553a2f23639dcab117939c` | `17d0358cbd2c7a8b5c3a725615802ff85db1c052` | 2026-07-27 | ci: report active-library completion instead of gating on it |
| `22ba5071780efafc65fc88445af8eb17e07f01cf` | `44515c3525400ea2c8dbaccb03f051589a817f34` | 2026-07-27 | ci: install libxdo-dev so the Linux desktop binary links |
| `899a47f4a93e46c800089e496961d4c270cb27a8` | `963c72ee99093dc72538b74f5a960a39268551cf` | 2026-07-27 | fix(windows): update bir-desktop to the windows 0.62 API |
| `ee505729166076490c65eb57fe8f5bccb4731de7` | `c38a3dcd4eea2ec820161817e13fd6c50be69d03` | 2026-07-27 | ci: narrow the CI visual-gate policy test; add Xvfb for GTK tests |
| `2c62060e26a5393cabd099875b7a3a7ce2455c8c` | `913bf621118b1b23a2da6c961bc836fd95f4d54d` | 2026-07-27 | fix(windows): stop canonicalizing the test temp dir; clear Windows clippy |
| `239540fc8c8e20aefe6ce347e05aa49f29c24a13` | `21cefbdd5e7f0fcb76ced37649e421e459fc1d15` | 2026-07-27 | fix(windows): drop no-op mem::forget; regenerate the pinned manifest |
| `6bb538f80e46b56dc088ed1a4dc16ba737e3f9f8` | `767c4201448dde56c741035471c7ff2e6c35a25a` | 2026-07-27 | fix(windows): correct temp-path form and a read-only sync_all in PDF export |
| `53ec4a67fe83b275d0dbfcbf9a75bd244ff6462b` | `06de2903c2d19c42f16d82d9fd18e4a1241606d7` | 2026-07-27 | fix(windows): correct the UNC skip guard and a pending-delete rename race |
| `9a43cdc021cf60194a40a39db82e1c18ffcc78ce` | `44a4de982028935494ee4af51330328bac51a63c` | 2026-07-27 | fix(windows): never reuse a deleted directory name in proposal cleanup teardown |
| `363036591a085c75b9d5ebba24140e07fd7a4c1e` | `bc66f03dfc1906dc6135caf183baae0435be5a19` | 2026-07-28 | fix(windows): keep proposal-cleanup renames inside the container's own parent |
| `8903f5988641ed55ed59176ad3872f735d223fb0` | `1b77fc929fa58edaea034ccbd74073905697b397` | 2026-07-28 | fix(windows): stop the identity test recreating an entry in the temp root |
| `64195655d8167609f678e364b5a08d3b231d2e7f` | `6985a198ceea1727a83027f7ee1c2a2eb4876a53` | 2026-07-28 | docs(validation): record the worktree cleanup inventory |
| `8cc89a17786712c4c267aaf015bb8ed2ea87cc63` | `71028aca8c4b276f2b3053032e92e67b7e5af020` | 2026-07-28 | docs: point the guidance at the single bir/main checkout |
