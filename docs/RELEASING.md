# CI, testy i wydania

## CI

Workflow **CI** (`.github/workflows/check.yml`) uruchamia się dla pushów na gałęzie, pull requestów i ręcznie. Jest też wywoływany przez proces wydania dla dokładnie tego samego commita co tag.

| Etap | Kontrole |
| --- | --- |
| Frontend and release tooling | actionlint, Prettier, zgodność wersji i changelog, Vitest z progami pokrycia, TypeScript/Svelte, build Vite |
| Rust and desktop | Windows / Ubuntu 22.04 / macOS: rustfmt, Clippy bez ostrzeżeń, testy Rust |
| Windows desktop | Build debug z osadzonym frontendem; test prawdziwego procesu Tauri i WebView2 |
| CI passed | Zbiorczy wymagany status; sukces tylko po sukcesie wszystkich poprzednich etapów |

Instalacja używa `npm ci`, a Cargo `--locked`. Oba lockfile muszą być commitowane. Cache zależności Rust i npm skraca kolejne przebiegi. Nowszy push anuluje starsze CI tej samej gałęzi. Wydania nie są anulowane w trakcie pracy.

Raport JUnit i pokrycie kodu są dostępne jako artefakt `frontend-test-results` przez 14 dni. Pokrycie obejmuje logikę paneli, formatowanie i narzędzia release; nie jest metryką pokrycia całego interfejsu. Progi: 85% instrukcji/linii, 80% gałęzi, 90% funkcji.

Test desktopowy ma timeouty i osobny profil WebView2. W razie błędu zapisuje log, tekst strony i — jeśli debugger jest osiągalny — screenshot do `test-results/desktop/`. CI udostępnia te pliki jako `windows-desktop-diagnostics`.

W ustawieniach repozytorium dla `main` można ustawić **CI passed** jako wymagany status przed merge. Nazwa będzie dostępna po pierwszym przebiegu workflow.

## Lokalne sprawdzenie

Wymagane: Node.js 22.12+ z linii 22 lub Node.js 24 LTS, Rust stable z `rustfmt` i `clippy`, zależności Tauri dla systemu.

```sh
npm ci
npm run format:check
npm run release:check
npm run test:coverage
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-targets
```

Test integracyjny na Windows:

```sh
npm run tauri build -- --debug --no-bundle --ci -- --locked
npm run test:desktop
```

`npm test` uruchamia testy bez pomiaru pokrycia; `npm run test:watch` działa w trybie obserwacji. `npm run format` i `cargo fmt --manifest-path src-tauri/Cargo.toml` poprawiają formatowanie.

## Przygotowanie wersji

Wersja musi być taka sama w:

- `package.json`,
- `package-lock.json` (nagłówek i pakiet główny),
- `src-tauri/tauri.conf.json`,
- `src-tauri/Cargo.toml`,
- `src-tauri/Cargo.lock` (tylko pakiet `mycmd`).

Skrypt aktualizuje je razem, nie zmieniając wersji zależności:

```sh
npm run release:version -- 0.2.0
```

Następnie przenieś odpowiednie wpisy z `Unreleased` w `CHANGELOG.md` pod `## [0.2.0] - YYYY-MM-DD`. Wykonaj `npm run release:check` i pozostałe testy. Zacommituj zmiany i wprowadź je do `main` przez CI.

Dla pierwszego wydania obecna wersja `0.1.0` i jej changelog są już przygotowane; nie trzeba jej ponownie podbijać.

## Uruchomienie wydania

Tag wskazuje commit zawierający workflow i wszystkie źródła wydania. Dla pierwszej wersji:

```sh
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "myCmd 0.1.0"
git push origin v0.1.0
```

Workflow **Release**:

1. Sprawdza zgodność tagu `vX.Y.Z` z manifestami i obecność changelogu.
2. Uruchamia pełne CI na tagowanym commicie.
3. Równolegle buduje pakiety:

   | Platforma | Architektura | Pakiety |
   | --- | --- | --- |
   | Windows | x64 | MSI, NSIS EXE |
   | Linux (Ubuntu 22.04) | x64 | DEB, AppImage |
   | macOS | Intel x64 | DMG |
   | macOS | Apple Silicon arm64 | DMG |

4. Sprawdza obecność wszystkich sześciu instalatorów i generuje `SHA256SUMS.txt`.
5. Tworzy **draft GitHub Release** z pakietami i opisem z changelogu.

Pakiety pośrednie są dostępne w artefaktach workflow przez 14 dni; pliki dołączone do GitHub Release nie mają tego terminu usunięcia. Prefiks nazwy pliku wskazuje platformę i architekturę.

Po sprawdzeniu draftu opublikuj go przyciskiem **Publish release**. Wersje z przyrostkiem, np. `0.2.0-rc.1`, automatycznie otrzymują flagę prerelease. Wszystkie platformy muszą się zbudować; błąd którejkolwiek blokuje przygotowanie release.

Workflow korzysta z wbudowanego `GITHUB_TOKEN`. Tylko końcowy job ma `contents: write`. Dodatkowe sekrety nie są potrzebne do obecnych, niepodpisanych pakietów. Certyfikaty Windows oraz podpisywanie/notaryzacja Apple nie są skonfigurowane.

## Powtórzenie i diagnoza

- Dla błędu testów lub kompilacji otwórz log joba i odpowiednie artefakty. Poprawki źródeł wydawaj pod nową wersją/tagiem.
- Przy przejściowym błędzie pobierania narzędzi użyj **Re-run failed jobs**.
- Można ręcznie uruchomić workflow na istniejącym tagu, np. `gh workflow run release.yml --ref v0.1.0`. Dispatch z gałęzi zostanie odrzucony.
- Ponowny przebieg aktualizuje istniejący draft i zastępuje jego pliki. Opublikowany release nie zostanie nadpisany; wymaga nowej wersji.

Sprawdzenie pobranych plików na Linux/macOS:

```sh
sha256sum --check SHA256SUMS.txt
# macOS bez sha256sum:
shasum -a 256 --check SHA256SUMS.txt
```

Na Windows: `Get-FileHash .\nazwa-instalatora.exe -Algorithm SHA256` i porównanie z wpisem w `SHA256SUMS.txt`.
