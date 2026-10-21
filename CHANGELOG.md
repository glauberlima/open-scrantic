# Changelog

## v0.4.0 — 2026-10-21

### Bug Fixes

- Average dithered colors in sRGB instead of linear light ([3beb217](https://github.com/glauberlima/open-scrantic/commit/3beb21785aaf2b9e5f3f48639aa64164427bb983))


### Documentation

- Reduce logo width to 240px ([2e8f6f2](https://github.com/glauberlima/open-scrantic/commit/2e8f6f210a712cfffed729199676e572943e15bb))
- Drop the spacer between logo and badges ([acdb713](https://github.com/glauberlima/open-scrantic/commit/acdb7133a531bbd5f7d15edf68cd65cd19481719))
- Move per-module notes into nested CLAUDE.md ([6a0617a](https://github.com/glauberlima/open-scrantic/commit/6a0617a611d88d4fa6edee6faa09eaa3553896fe))
- Note the legacyScreenSaver host on macOS ([04c8f42](https://github.com/glauberlima/open-scrantic/commit/04c8f4211c0e711e6d7a2290a74b4901cf17b1fc))
- Record how the original plays sounds ([d3aeb4a](https://github.com/glauberlima/open-scrantic/commit/d3aeb4a5a99e06f504030393e4f4807a06f27817))
- Record how the original presents the frame ([6b8256c](https://github.com/glauberlima/open-scrantic/commit/6b8256ca63368773b9893e530e6e4ba995b6ecb4))
- Note that the original's MCI call is dead code ([04dd0a7](https://github.com/glauberlima/open-scrantic/commit/04dd0a7cc8d74a0b985190a27a6d0301bfea62f7))
- Record where the original plays sounds ([fb4db8d](https://github.com/glauberlima/open-scrantic/commit/fb4db8d12919d5ab9ce436993d7fcce76f849cb9))
- Add diagrams of the architecture and the main loop ([0c59259](https://github.com/glauberlima/open-scrantic/commit/0c592599cb0da461a91faf4755757c60f7f04bb9))
- Waves only controls the ocean whitecaps ([9c38b5d](https://github.com/glauberlima/open-scrantic/commit/9c38b5d6ec17130cbd75a20dfe128308c12862ea))
- Document the holidays of the original ([dc3acdb](https://github.com/glauberlima/open-scrantic/commit/dc3acdb6f07d731f913cde3cc0257a5ca04905a2))
- Add an image of the four holiday decorations ([5c534e8](https://github.com/glauberlima/open-scrantic/commit/5c534e8ce5344f985adedcb0051120b4aabfeeec))
- Record the hidden and unused content of the original ([3b9fa9d](https://github.com/glauberlima/open-scrantic/commit/3b9fa9d0cc9a2cd9e4fa5db7ccbe6671cd820d49))
- Simplify the note on the unscheduled STAND.ADS scene ([15b620b](https://github.com/glauberlima/open-scrantic/commit/15b620bb4399dc0aec0b1d90b6f5e62d953460ef))
- Add a contributing guide and a code of conduct ([abd0e60](https://github.com/glauberlima/open-scrantic/commit/abd0e60de160f139661a1bfa0b2501fa5c00e985))
- Link the contributing guide and the diagrams, mention Rust ([d53b48b](https://github.com/glauberlima/open-scrantic/commit/d53b48b102aeca6ddba78662f974168cfa14af78))
- Sync architecture and engine notes with the code ([2bebf2b](https://github.com/glauberlima/open-scrantic/commit/2bebf2b756f808662bf8b195cab26fe14836cb90))


### Features

- Scale the frame with sharp-bilinear sampling ([ea138be](https://github.com/glauberlima/open-scrantic/commit/ea138be10909ab72dc7de49645858c8be104f067))
- Use the driver's default buffer outside Linux ([bea6161](https://github.com/glauberlima/open-scrantic/commit/bea61617d5808d77e6003aabb42488c860592742))
- Override the INI options from the command line ([9b64493](https://github.com/glauberlima/open-scrantic/commit/9b64493a6248b62fc46c49073125f20d1c92887a))
- Set the calendar day with DATE=MMDD ([4c8e120](https://github.com/glauberlima/open-scrantic/commit/4c8e1203ad851bcab28ffdf832baaeda09a51d9a))
- Center the window on the primary monitor ([d67c04b](https://github.com/glauberlima/open-scrantic/commit/d67c04b340b3588937d3f7f7e01045779d6d4fd2))

## v0.3.0 — 2026-10-18

### Bug Fixes

- Use a larger device buffer and reopen the device after stream errors ([278f4bd](https://github.com/glauberlima/open-scrantic/commit/278f4bd303516bf7b59320872dd02e768b37b13c))
- Dedither one-pixel alternating lines at the edge of dithered areas ([330092c](https://github.com/glauberlima/open-scrantic/commit/330092c7371abf4e115c501e07889b914a3a30a1))


### Documentation

- Describe run dedithering and the audio buffer ([b5cb094](https://github.com/glauberlima/open-scrantic/commit/b5cb0943c7022d968e34ee6f57a82e76794135d2))


### Features

- Save a screenshot with F12 ([33111da](https://github.com/glauberlima/open-scrantic/commit/33111da3698aad3ff3dc539b3258be8276b61364))

## v0.2.1 — 2026-10-18

### Documentation

- Use a transparent, optimized logo at half the display size ([569ee39](https://github.com/glauberlima/open-scrantic/commit/569ee39aa3849aeec27c24ae454a29362fed7677))


### Performance

- Iterate scene order by index instead of cloning it ([650097e](https://github.com/glauberlima/open-scrantic/commit/650097ea787a11948aa14d96a0b740b53f0aa05b))
- Make SceneDef Copy to avoid a per-step clone ([3bc7b33](https://github.com/glauberlima/open-scrantic/commit/3bc7b33680be0f3945d7d2960ba43f379f9e0610))


### Refactoring

- Name scene flags, dedupe walk/scene completion, cache debug flag ([f8505bc](https://github.com/glauberlima/open-scrantic/commit/f8505bc38de5e98df0521d8d6d92dbd6f54e52ab))
- Share clip visibility and row-range helpers ([5a99657](https://github.com/glauberlima/open-scrantic/commit/5a99657a35e2ccd2bed520e0715ead16cc523340))
- Add Slot::new/tick and wave_rect helpers ([733d351](https://github.com/glauberlima/open-scrantic/commit/733d351f87660c115de7034600b41721504872d3))
- Name tag state constants ([3a5cc25](https://github.com/glauberlima/open-scrantic/commit/3a5cc25b4cc9a8cbaeb042585d9335f174401e0d))
- Share byte readers between exe and resource parsers ([6eab416](https://github.com/glauberlima/open-scrantic/commit/6eab416b1f7756b991b4f702830b7900e9b59dbb))

## v0.2.0 — 2026-10-18

### Bug Fixes

- Use the six repeat weights for repeatable scenes ([32a4f13](https://github.com/glauberlima/open-scrantic/commit/32a4f1325f711889ead9ea957f9ca79dc34c82c3))


### Documentation

- Widen the README logo to 640 ([9733c79](https://github.com/glauberlima/open-scrantic/commit/9733c7960e053e5128c107836d47c0ceedd84219))
- Add the "why" and a demo video ([cdcf8fe](https://github.com/glauberlima/open-scrantic/commit/cdcf8fe79c5397e200aea0715617860e511305ef))
- Say screensaver instead of game and link the demo video ([2bdbd87](https://github.com/glauberlima/open-scrantic/commit/2bdbd87af4cd9641dd33739ba4681c0273f095aa))
- Describe present.rs, --dedither and --soft-audio ([0362698](https://github.com/glauberlima/open-scrantic/commit/036269836f59fcaf397e81dc61a1e496af718441))
- Fix the release year of the original to 1992 ([0d31acc](https://github.com/glauberlima/open-scrantic/commit/0d31acce24b7c75cfd15c2504ec0c9b614b98c24))


### Features

- Add optional dedithering (--dedither) ([be15eea](https://github.com/glauberlima/open-scrantic/commit/be15eea4669ec1285474f37d9a655898fdb9279d))
- Add --soft-audio to resample, low-pass and fade the sounds ([2fb95dd](https://github.com/glauberlima/open-scrantic/commit/2fb95dd9d1c373393081e6ee501ae53658f28d83))


### Refactoring

- Rename GameData to ScreensaverData ([471632f](https://github.com/glauberlima/open-scrantic/commit/471632f58bd39976d8ab207c5acd699bf20eebd7))

## v0.1.1 — 2026-10-17

### Bug Fixes

- Harden asset loading against truncated or corrupt files ([5cff2cc](https://github.com/glauberlima/open-scrantic/commit/5cff2ccb621579ef6597010824bc2f8ffed69d63))
- Avoid a crash, a busy loop and a hang ([b88b1af](https://github.com/glauberlima/open-scrantic/commit/b88b1afda9834ed948fa70e25339447a6fb65acb))


### Documentation

- Rewrite README for end users ([4f6c8d2](https://github.com/glauberlima/open-scrantic/commit/4f6c8d2f29017ee5f7725b39c1018dbda5658551))
- Call it a screensaver and add an AI disclosure ([3cf9697](https://github.com/glauberlima/open-scrantic/commit/3cf9697a380139d836e6951456ede1516211687f))
- Tidy README, update logo and footer ([1c22ffa](https://github.com/glauberlima/open-scrantic/commit/1c22ffa9b145142a4bcade2362162b09add7a2c1))
- Add intro paragraph and flat badges ([1be1301](https://github.com/glauberlima/open-scrantic/commit/1be13011b20bff4008af80131e86eb12ea645bbc))
- Widen README logo ([d5ce2de](https://github.com/glauberlima/open-scrantic/commit/d5ce2de0c449030a43d430200b8f98c3ac89044f))
- Add ARCHITECTURE.md, fix SCRANTIC.EXE naming and add package metadata ([920153a](https://github.com/glauberlima/open-scrantic/commit/920153a8e263216456961eb0f817bcef0e02b747))
- Update the wider logo ([5c5ad9d](https://github.com/glauberlima/open-scrantic/commit/5c5ad9d60a4962138a8a1169705bc6478bbeab40))
- Add spacing around the README logo ([8fa5114](https://github.com/glauberlima/open-scrantic/commit/8fa511463a1e6d5610b513972512e0ce2c2a1482))
- Fix spacing around the README logo ([2c22dbf](https://github.com/glauberlima/open-scrantic/commit/2c22dbf5bae707cd4b6d5271c21b56b62024ab2d))
- Add island emoji to the README title ([400ee47](https://github.com/glauberlima/open-scrantic/commit/400ee47fdab4c806e778430964d3370e4d9c880e))
- Add SECURITY.md, document original quirks and polish wording ([30866ca](https://github.com/glauberlima/open-scrantic/commit/30866ca056698f8cc8fca063f4956677b3b09c9d))
- Drop "faithful" from the project description ([f8c65f6](https://github.com/glauberlima/open-scrantic/commit/f8c65f653748e7e0a8f20c921d7d3f54cdf1f313))
- Use a static license badge ([a8cb0b3](https://github.com/glauberlima/open-scrantic/commit/a8cb0b38cc052961a5da01af3c30ffe85777d4fa))


### Performance

- Avoid per-frame copies in the TTM interpreter ([bde010e](https://github.com/glauberlima/open-scrantic/commit/bde010e1bd27d5a74b828cc2991a3957cf7acc45))

## v0.1.0 — 2026-10-17

### Breaking Changes

- Always run as fullscreen screensaver, drop all CLI options ([e7408bd](https://github.com/glauberlima/open-scrantic/commit/e7408bd76d819e9383070f1e1478974bf50065b3))


### Bug Fixes

- Save the 0xA054 zone from the background ([3ae11c5](https://github.com/glauberlima/open-scrantic/commit/3ae11c57501851a37d9a9db31518fe3d4f20ca1d))


### Documentation

- Translate README and engine notes to en-US ([bd4966c](https://github.com/glauberlima/open-scrantic/commit/bd4966c1a1a40d8f546bf4f4ec4fa58ccb5ee3e1))
- Add CLAUDE.md with build commands and architecture overview ([943be3b](https://github.com/glauberlima/open-scrantic/commit/943be3b5908d8da0b6abb820f46bed4dcc3e3f76))
- Credit REA for decompilation ([c59fbe5](https://github.com/glauberlima/open-scrantic/commit/c59fbe511ac4d40f1090d031a5568eb6216814f2))
- Expand README with quick start, INI reference, FAQ and legal notice ([4c1372e](https://github.com/glauberlima/open-scrantic/commit/4c1372e77eb58a8c7822d48214106744e3a91f44))
- Add MIT license ([8a758fe](https://github.com/glauberlima/open-scrantic/commit/8a758fef582f9190b016badc2277867c3f2c88de))
- Add agent-agnostic AGENTS.md ([9f7b5f3](https://github.com/glauberlima/open-scrantic/commit/9f7b5f3598faf536610696171e7f31aa95fa8d9e))
- Add logo and tighten README ([f9897a6](https://github.com/glauberlima/open-scrantic/commit/f9897a62a0e29a58ac5fcfa8001e153bf4ecba82))
- Add SHA-256 checksums of the original screensaver files ([67b8a9d](https://github.com/glauberlima/open-scrantic/commit/67b8a9dfcb481f3407d9e96d28f7035945c02759))


### Features

- Reimplement Johnny Castaway engine in Rust ([ab74bc4](https://github.com/glauberlima/open-scrantic/commit/ab74bc47388061ea57f5f580f0d5b352dac73ecd))
- Add scene forcing to the snapshot example ([e17bcf6](https://github.com/glauberlima/open-scrantic/commit/e17bcf638f3d922626b0ce5984a2dde4e9e0e7a9))
- Read screensaver files and SCRANTIC.INI from the executable directory ([d0140d1](https://github.com/glauberlima/open-scrantic/commit/d0140d12147fed3bfcae23778c71cb02826031e6))

