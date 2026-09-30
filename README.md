# myCmd

[![CI](https://github.com/pankleks/myCmd/actions/workflows/check.yml/badge.svg)](https://github.com/pankleks/myCmd/actions/workflows/check.yml)
[![Release](https://github.com/pankleks/myCmd/actions/workflows/release.yml/badge.svg)](https://github.com/pankleks/myCmd/actions/workflows/release.yml)

Dwupanelowy menedżer plików desktopowych zgodny z zakresem MVP w `design.md`.
Frontend: Svelte 5 / TypeScript / Vite. Backend: Tauri 2 / Rust.

## Uruchomienie

Wymagane: Node.js 22.12+ z linii 22 lub Node.js 24 LTS, Rust stable oraz [zależności systemowe Tauri](https://v2.tauri.app/start/prerequisites/).
Na Windows potrzebne są MSVC Build Tools (C++) i WebView2; na Linux WebKitGTK 4.1 oraz biblioteki wymienione w dokumentacji Tauri; na macOS Xcode Command Line Tools.

```sh
npm install
npm run tauri dev
```

```sh
npm run build                 # sprawdzenie TypeScript/Svelte + frontend produkcyjny
npm run tauri build           # aplikacja desktopowa i pakiety instalacyjne
cargo test --manifest-path src-tauri/Cargo.toml
```

Test integracyjny Windows (Node.js 22+), uruchamiający rzeczywisty proces Tauri/WebView2 i operujący wyłącznie na tymczasowych danych:

```sh
npm run tauri build -- --debug --no-bundle
npm run test:desktop
```

Po takim buildzie aplikacja znajduje się w `src-tauri/target/debug/mycmd.exe`.

`npm run dev` uruchamia sam frontend. Dostęp do systemu plików wymaga procesu Tauri; w przeglądarce wyświetlany jest odpowiedni komunikat.

## Obsługa

| Skrót                             | Działanie                                                                   |
| --------------------------------- | --------------------------------------------------------------------------- |
| ↑ / ↓, Home / End, Page Up / Down | Kursor                                                                      |
| Enter / dwuklik                   | Otwórz katalog lub plik w aplikacji systemowej                              |
| Backspace                         | Katalog nadrzędny                                                           |
| Tab                               | Drugi panel                                                                 |
| Alt+litera                        | Szybkie wyszukiwanie w aktywnym panelu (Esc zamyka)                         |
| Alt+F1 / Alt+F2                   | Wybór dysku w lewym / prawym panelu                                         |
| Space / Insert                    | Zaznaczenie (Insert przesuwa kursor; spacja na katalogu liczy jego rozmiar) |
| *                                 | Odwróć zaznaczenie bieżącej listy                                           |
| + / -                             | Dodaj / usuń elementy pasujące do maski glob (domyślnie `*.*`)              |
| Ctrl+A / Esc                      | Zaznacz wszystkie / wyczyść zaznaczenie                                     |
| Ctrl+klik / Shift+klik            | Zaznaczenie wielu elementów / zakresu                                       |
| F2                                | Zmień nazwę jednego elementu                                                |
| F5 / F6                           | Kopiuj / przenieś do drugiego panelu                                        |
| F7                                | Utwórz katalog                                                              |
| F8 / Delete                       | Przenieś do Kosza po potwierdzeniu                                          |
| Shift+F8 / Shift+Delete           | Usuń trwale po potwierdzeniu                                                |
| F9                                | Ustawienia (font, ukryte pliki, pasek przycisków)                           |
| Ctrl+H                            | Pokaż/ukryj pliki ukryte                                                    |
| Ctrl+I                            | Otwórz ten sam katalog w drugim panelu                                      |
| Ctrl+R                            | Odśwież panel                                                               |
| Ctrl+L                            | Edytuj ścieżkę                                                              |
| Ctrl++ / Ctrl+- / Ctrl+0          | Większy / mniejszy / domyślny font listy plików                             |

Kliknięcie nagłówka sortuje kolumnę. Katalogi pozostają na początku. Każdy panel ma niezależne sortowanie i zaznaczenie; widoczność plików ukrytych jest wspólna dla obu paneli (Ctrl+H lub F9). Lista jest wirtualizowana. Szerokość kolumn (wspólna dla obu paneli – resize jednego resizuje drugi) zmienisz myszą (uchwyt między nagłówkami) albo strzałkami po przejściu Tabem na uchwyt; dwuklik na nagłówku resetuje szerokości. Zwykłe pisanie trafia do pola komend (i przenosi tam fokus). Alt+litery przy fokusie w polu komend lub ścieżki wpisują polskie znaki do tego pola. Alt+litery (lewy lub prawy) przy fokusie na panelu plików otwierają szybkie wyszukiwanie.

## Obliczanie rozmiarów katalogów

Podczas obliczania rozmiarów katalogów przycisk „Cancel sizing” anuluje aktywne pomiary w obu panelach. Przerwanie jest sprawdzane między wpisami systemu plików; nie przerywa już trwającego blokującego wywołania systemowego (np. na niedostępnym udziale sieciowym). Anulowany katalog można zmierzyć ponownie.

## Test przenoszenia między woluminami

Test faktycznego przenoszenia między systemami plików (Unix) jest domyślnie pomijany, ponieważ wymaga drugiego zamontowanego woluminu. Można go uruchomić poleceniem `MYCMD_TEST_TRANSFER_VOLUME=/ścieżka/do/woluminu cargo test --manifest-path src-tauri/Cargo.toml --locked cross_device_move_uses_real_copy_and_remove_fallback -- --ignored`. Test sprawdza różne identyfikatory urządzeń i tworzy wyłącznie własne katalogi tymczasowe; nie używa istniejących plików na woluminie.

## Podgląd obrazów

Markdown dopuszcza maksymalnie 24 obrazy, 12 MiB danych obrazów i 32 miliony pikseli łącznie. Budżet pikseli korzysta z wymiarów odczytanych przez Rust przed dekodowaniem obrazu w WebView; dla GIF uwzględnia rozmiar płótna × liczbę klatek.

Animacja jest obsługiwana tylko w GIF: maksymalnie 100 klatek oraz 64 miliony pikseli łącznie na plik (rozmiar płótna × liczba klatek). Klatki są liczone bez dekodowania pikseli; odrzucane są też opisy klatek wychodzących poza płótno. Animowane PNG, WebP i sekwencje AVIF oznaczone marką `avis` są odrzucane; można je otworzyć w aplikacji systemowej. Statyczne obrazy w tych formatach nadal są obsługiwane. Limity nie stanowią pełnego ograniczenia pamięci WebView.

Podgląd obrazów (F3 lub panel Shift+F3) i lokalnych obrazów w Markdown ma limity na pojedynczy plik: 4 MiB danych, 16 milionów pikseli oraz 16384 piksele na każdy wymiar. Wymiary są odczytywane z nagłówka bez dekodowania całego obrazu. Uszkodzone lub nieczytelne nagłówki są odrzucane. Są to ograniczenia rozmiaru obrazu, nie pełny limit pamięci WebView ani liczby klatek animacji; plik odrzucony można otworzyć w aplikacji systemowej.

## Archiwa i dostawcy zawartości

Enter lub dwuklik na lokalnym pliku `.zip` / `.7z` otwiera jego zawartość w panelu. Katalogi wewnątrz archiwum działają jak zwykłe katalogi; `..` i Backspace wracają wyżej, a z korzenia archiwum do katalogu na dysku. F5 wyodrębnia plik pod kursorem lub wszystkie zaznaczone pliki/katalogi do lokalnego katalogu w drugim panelu (cel można zmienić w dialogu). Ekstrakcja zachowuje zawartość katalogów, obsługuje konflikty i anulowanie; wykorzystuje prywatny katalog tymczasowy i limit 8 GiB rozpakowanych danych na operację. Archiwum pozostaje niezmienione. Otwieranie plików, podgląd i modyfikacje archiwów nie są jeszcze obsługiwane. Nie trzeba instalować 7-Zip. Archiwa wymagające hasła nie są obsługiwane przy ekstrakcji.

Warstwa dostawców zawartości (`src/filesystem/providers.ts`) implementuje lokalny system plików i archiwa. Typy lokalizacji rezerwują FTP, SFTP, SMB/Samba, NFS oraz wyniki wyszukiwania; ich dostawcy nie są jeszcze implementowani. Zamontowane udziały sieciowe nadal działają przez lokalny system plików.

F8/Delete wewnątrz archiwum trwale usuwa element pod kursorem lub wszystkie zaznaczone elementy (katalogi razem z zawartością), po potwierdzeniu. Operacja tworzy i weryfikuje nową wersję archiwum, a następnie zastępuje oryginał; błędy i anulowanie przed zastąpieniem pozostawiają oryginał bez zmian. ZIP zachowuje surowe dane pozostałych wpisów; 7z jest ponownie kompresowany. Zaszyfrowane archiwa i nieobsługiwane metody kompresji mogą uniemożliwić weryfikację i usuwanie. Weryfikacja/przepisanie ma limit 8 GiB danych rozpakowanych.

## Konfiguracja

Polecenia powłoki są wykonywane tylko raz, mają limit czasu 120 sekund i przycisk anulowania. Przechwycone wyjście jest ograniczone do 1 MiB na każdy strumień (stdout/stderr); dalsze dane są odczytywane i odrzucane, aby nie blokować procesu. Anulowanie i timeout próbują zakończyć grupę procesów na Unix oraz drzewo procesu przez `taskkill /T /F` na Windows. To nie jest sandbox: odłączone procesy mogą uniknąć zakończenia, a wykonane wcześniej zmiany nie są cofane. Oczekiwanie na zamknięcie strumieni potomków jest ograniczone, a już odczytane dane są zachowywane z ostrzeżeniem o niepełnym wyjściu. Nieblokujący odczyt na Unix oraz sprawdzanie dostępnych danych przez `PeekNamedPipe` na Windows pozwalają zakończyć wątki czytające po tym limicie, nawet gdy potomek zachowa otwarty strumień. Implementacja Windows wymaga potwierdzenia testami natywnymi w CI.

Aplikacja zapisuje automatycznie (z opóźnieniem ~250 ms) plik `config.json`:

| System  | Lokalizacja                                       |
| ------- | ------------------------------------------------- |
| Windows | `%APPDATA%\mycmd\config.json`                     |
| Linux   | `~/.config/mycmd/config.json`                     |
| macOS   | `~/Library/Application Support/mycmd/config.json` |

Zapisywane są: aktualne katalogi obu paneli, wspólna szerokość kolumn, rozmiar fontu listy plików (12–24 px, domyślnie 18), wspólne pokazywanie plików ukrytych (domyślnie wyłączone) oraz widoczność dolnego paska przycisków funkcyjnych (domyślnie włączony). Font, ukryte pliki i pasek przycisków zmienisz w oknie ustawień (F9). Przy starcie aplikacja odtwarza katalogi (nieistniejące zastępuje katalogiem domowym), kolumny i font. Uszkodzony lub brakujący plik oznacza domyślne ustawienia. Zmienna środowiskowa `MYCMD_CONFIG_DIR` nadpisuje katalog konfiguracji (używana m.in. przez testy).

## Operacje i architektura

- `src/state/commander.svelte.ts`: stan paneli w runach Svelte 5.
- `src/state/preferences.svelte.ts`: preferencje (font, kolumny) z autozapisem.
- `src/operations/controller.svelte.ts`: cykl życia operacji, filtrowanie zdarzeń po ID, konflikty i anulowanie; blokada nowych operacji trwa do zakończenia odświeżenia paneli.
- `src/components/FilePreview.svelte`: wspólny podgląd tekstu, Markdown i obrazów; niezależne kontenery `FileViewerDialog.svelte` oraz `FilePreviewPanel.svelte`.
- `src/utils/markdownPreview.ts`: sanitizacja Markdown i ograniczone ładowanie lokalnych obrazów, testowane w środowisku DOM.
- `src/utils/config.ts`: normalizacja i walidacja konfiguracji.
- `src/filesystem/api.ts`: typowany most IPC; frontend nie wykonuje operacji systemu plików.
- `src-tauri/src/commands.rs`: cienkie komendy IPC.
- `src-tauri/src/config.rs`: odczyt/zapis `config.json` (m.in. `MYCMD_CONFIG_DIR`).
- `src-tauri/src/filesystem.rs`: listowanie, metadane, korzenie/montowania, walidacja nazw.
- `src-tauri/src/operations.rs`: wspólny model operacji, zadania w tle, konflikty, postęp i anulowanie.
- `src-tauri/src/error.rs`: błędy strukturalne.

Operacje używają zaznaczenia lub elementu pod kursorem. Jednocześnie interfejs uruchamia jedną operację. Konflikty można pomijać, automatycznie/ręcznie zmieniać nazwę, nadpisywać lub anulować; dostępna jest reguła dla całej operacji. Nadpisanie katalogów scala zawartość, a konflikty wewnątrz obsługuje ten sam mechanizm. Nadpisanie niezgodnych typów oraz docelowych dowiązań jest odrzucane — należy wybrać pominięcie lub zmianę nazwy.

Pliki są kopiowane porcjami do plików tymczasowych w katalogu docelowym i publikowane dopiero po zakończeniu zapisu. Przenoszenie używa natywnej zmiany położenia; po błędzie między systemami plików stosuje kopiowanie i usunięcie źródła. Pominięte źródła pozostają na miejscu. Anulowanie pozostawia ukończone elementy i usuwa bieżący tymczasowy plik; nie cofa wcześniejszych zmian. Usuwanie jest trwałe.

Kopiowanie zachowuje uprawnienia oraz czasy dostępu i modyfikacji plików; nowe katalogi otrzymują metadane po skopiowaniu zawartości. Na Unix kopiowane są również dostępne rozszerzone atrybuty. Błąd zachowania tych metadanych przerywa transfer przed usunięciem źródła. Scalanie z istniejącym katalogiem nie zastępuje jego metadanych. Transfer weryfikuje tożsamość, rozmiar i czas modyfikacji źródłowego pliku przed publikacją oraz przed usunięciem; na Unix sprawdza też ctime. Wykryta zmiana pozostawia źródło i zgłasza `source_changed`; jeśli cel został już opublikowany, pozostają obie kopie.

Nie jest to snapshot systemu plików: zapis współbieżny tuż po ostatniej weryfikacji nadal może wystąpić. Nie należy przenosić aktywnie zapisywanych plików. Kopiowanie między woluminami nie gwarantuje zachowania właściciela, wszystkich ACL, czasu utworzenia, relacji twardych dowiązań ani alternatywnych strumieni Windows. Dla danych wymagających pełnego zachowania tych właściwości użyj natywnego narzędzia backupu lub przenoszenia.

Dowiązania są rozpoznawane, kopiowane jako dowiązania i nie są śledzone w rekursji. Utworzenie dowiązania na Windows może wymagać włączonego Developer Mode lub odpowiednich uprawnień. Ścieżki są przetwarzane w Rust jako `PathBuf`, z kanonikalizacją katalogów (w tym ścieżek UNC i długich ścieżek Windows). Ścieżki niepoprawne w Unicode są odrzucane przy listowaniu zamiast udostępniania niejednoznacznych nazw.

Wpisy, które zniknęły podczas odczytu, są nieczytelne lub mają nazwy niepoprawne w Unicode, nie blokują listowania pozostałej zawartości. Panel pokazuje liczbę pominiętych wpisów i rozwijane szczegóły (maksymalnie 20 ostrzeżeń). Błąd otwarcia całego katalogu nadal zachowuje poprzednią listę i pokazuje błąd.

## Weryfikacja platform

Testy backendu obejmują rekursję, konflikty, przenoszenie, blokadę kopiowania katalogu do siebie, anulowanie i walidację nazw; na Unix również pętle dowiązań. Skonfigurowano CI dla buildu frontendu oraz testów Rust na Windows, Linux i macOS.

Lokalnie zweryfikowano Windows: build frontendu bez błędów/ostrzeżeń, 5 testów Rust, build aplikacji oraz test integracyjny startu, listowania, kopiowania, dialogu konfliktu, zmiany nazwy, tworzenia katalogu, przenoszenia, usuwania, zdarzeń postępu i obsługi F7. W przeglądarce sprawdzono wirtualizację 10 000 wpisów, zaznaczanie i przełączanie paneli. Linux/macOS, udziały UNC i przenoszenie między fizycznymi woluminami wymagają weryfikacji na docelowych środowiskach.

## CI i wydania

- Push/PR: testy Vitest z pokryciem, kontrola TypeScript/Svelte i formatowania, testy Rust oraz Clippy na trzech systemach; test desktopowy na Windows.
- `npm test` — testy logiki paneli i narzędzi wydania; `npm run test:coverage` — pokrycie i JUnit.
- `npm run release:version -- 0.2.0` — aktualizacja wersji we wszystkich manifestach i lockfile.
- `npm run release:check` — kontrola zgodności wersji i changelogu.
- Tag `vX.Y.Z`: pełne CI, instalatory dla Windows/Linux/macOS Intel i ARM, sumy SHA-256 oraz szkic GitHub Release.

Pełna instrukcja: [docs/RELEASING.md](docs/RELEASING.md). Historia zmian: [CHANGELOG.md](CHANGELOG.md).
